mod config;
mod upload;
mod xml;

use clap::{Parser, Subcommand};
use config::*;
use log::{info, debug};
use std::fs;
use std::io::Cursor;
use std::ffi::OsStr;
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs::File as TokioFile;
use uuid::Uuid;
use chrono::{Utc, DateTime, NaiveDate};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Directory that contains the channel.yaml file
    #[clap(short, long, value_parser)]
    channel_file: PathBuf,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate episode config
    CreateEpisode(NewEpisode),
    /// Render XML that would be uploaded to S3 storage
    RenderChannel(RenderOptions),
}

#[derive(Parser)]
struct RenderOptions {
    /// When set, the xml file will be uploaded instead of written to stdout
    #[clap(long, short, action)]
    upload: bool,
}

#[derive(Parser)]
struct NewEpisode {
    /// mp3 file for the episode
    #[clap(value_parser)]
    file: PathBuf,
    /// URL for the episode
    #[clap(short, long)]
    date: String,
    /// Episode Name
    #[clap(short, long)]
    title: String,
}

#[derive(Error, Debug)]
pub enum CliError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    YamlError(#[from] serde_yaml::Error),
    #[error(transparent)]
    XmlError(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    S3UploadError(#[from] rusoto_core::RusotoError<rusoto_s3::PutObjectError>),
    #[error("Error processing MP3 {0}")]
    Mp3Error(String),
    #[error(transparent)]
    ChronoError(#[from] chrono::ParseError),
    #[error("unknown data store error")]
    Unknown,
}

fn main() -> Result<(), CliError> {
    human_panic::setup_panic!();
    dotenv::dotenv().ok();
    env_logger::init();
    let cli = Cli::parse();

    if !cli.channel_file.exists() {
        panic!("'{:?}' doesn't exist.", cli.channel_file);
    }

    let mut episode_dir = cli.channel_file.clone();
    episode_dir.pop();
    episode_dir.push("episodes");

    let channel_file_text = fs::read_to_string(cli.channel_file)?;
    let channel_config = serde_yaml::from_str(&channel_file_text)?;

    info!("Channel Config: {:?}", channel_config);

    parsed_main(episode_dir, channel_config, cli.command)
}

#[tokio::main]
async fn parsed_main(
    episode_dir: PathBuf,
    channel_config: ChannelConfig,
    commands: Commands,
) -> Result<(), CliError> {
    match commands {
        Commands::RenderChannel(data) => render_xml(episode_dir, channel_config, data).await,
        Commands::CreateEpisode(data) => create_episode(episode_dir, channel_config, data).await,
    }
}

async fn create_episode(
    episode_dir: PathBuf,
    channel_config: ChannelConfig,
    data: NewEpisode,
) -> Result<(), CliError> {

    let publish_date = NaiveDate::parse_from_str(&data.date, "%Y-%m-%d")?;
    let publish_date: DateTime<Utc> = DateTime::from_utc(publish_date.and_hms(0,0,0), Utc);
    let publish_name = publish_date.format("%Y-%m-%d").to_string();

    let object_key = format!(
        "{}/artifacts/{}.mp3",
        channel_config.publishing.prefix, publish_name
    );

    let file = TokioFile::open(&data.file).await?;
    let file_metadata = file.metadata().await?;
    let size = file_metadata.len();

    let upload_url = upload::upload_contents(
        file,
        size,
        channel_config.publishing.region,
        channel_config.publishing.bucket.clone(),
        object_key,
    )
    .await?;
    println!("Uploaded file {}", upload_url);

    let metadata = match mp3_metadata::read_from_file(&data.file) {
        Err(e) => return Err(CliError::Mp3Error(format!("{}", e))),
        Ok(metadata) => metadata,
    };
    let duraction = metadata.duration;

    let mut episode = Episode {
        id: Uuid::new_v4().to_string(),
        title: data.title.clone(),
        description: "Fill me in".into(),
        summary: "Fill me in".into(),
        link: Some("Fill me in, or delete me".into()),
        released_at: publish_date,
        season: 1,
        episode_number: 0,
        image: channel_config.channel.image.clone(),
        media: EpisodeMedia {
            url: upload_url,
            duration: duraction.as_secs(),
            bytes: size,
        },
        keywords: channel_config.channel.keywords.clone(),
    };

    update_episode_numbers(&mut episode, &episode_dir)?;

    info!("episode {:?}", episode);

    let yaml = serde_yaml::to_string(&episode)?;

    let mut episode_file = episode_dir.clone();
    episode_file.push(format!("{}-session.yaml", publish_name));

    fs::write(episode_file, yaml)?;

    Ok(())
}

fn update_episode_numbers(episode: &mut Episode, episode_dir: &PathBuf) -> Result<(), CliError> {
    let episodes: Vec<Episode> = get_all_episodes(episode_dir)?;

    let mut season_number = 0;
    let mut episode_number = 0;

    for episode in episodes {
        if season_number <= episode.season {
            season_number = episode.season;

            if episode_number <= episode.episode_number {
                episode_number = episode.episode_number;
            }
        }
    }

    episode.season  = season_number;
    episode.episode_number = episode_number + 1; 

    Ok(())
}

fn get_all_episodes(episode_dir: &PathBuf) -> Result<Vec<Episode>, CliError> {
    let paths = fs::read_dir(episode_dir)?;
    let mut episodes: Vec<Episode> = Vec::new();

    for path in paths {
        let path = path?.path();
        if path.extension() == Some(OsStr::new("yaml")) {
            debug!("Found episode {:?}", path);
            let text = fs::read_to_string(path)?;
            let episode: Episode = serde_yaml::from_str(&text)?;
            episodes.push(episode);
        }
    }

    Ok(episodes)
}

async fn render_xml(
    episode_dir: PathBuf,
    channel_config: ChannelConfig,
    render_options: RenderOptions,
) -> Result<(), CliError> {
    let episodes: Vec<Episode> = get_all_episodes(&episode_dir)?;

    debug!("List episodes {:?}", episodes);

    let rendered_podcast = xml::generate_podcast_xml(channel_config.channel, episodes)?;

    if render_options.upload {
        let object_key = format!("{}/podcast.xml", channel_config.publishing.prefix);
        let size = rendered_podcast.len();
        let read = Cursor::new(rendered_podcast.into_bytes());
        let url = upload::upload_contents(
            read,
            size.try_into().unwrap(),
            channel_config.publishing.region,
            channel_config.publishing.bucket,
            object_key,
        )
        .await?;

        println!("Podcast URL: {}", url);
    } else {
        println!("{}", rendered_podcast);
    }

    Ok(())
}
