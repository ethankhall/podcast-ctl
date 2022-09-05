mod config;
mod upload;
mod xml;

use clap::{Parser, Subcommand};
use config::*;
use log::info;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use thiserror::Error;
use tokio::fs::File as TokioFile;
use uuid::Uuid;

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
    /// Upload an mp3 to S3 storage
    UploadEpisodeRecording(UploadEpisodeRecording),
    /// Generate episode config
    CreateEpisode(NewEpisode),
    /// Render XML that would be uploaded to S3 storage
    RenderChannelXml(RenderOptions),
}

#[derive(Parser)]
struct RenderOptions {
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
    url: String,
    /// Episode Name
    #[clap(short, long)]
    name: String,
}

#[derive(Parser)]
struct UploadEpisodeRecording {
    /// mp3 file for the episode
    #[clap(value_parser)]
    file: PathBuf,
    /// Episode Name
    #[clap(short, long)]
    name: String,
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
        Commands::UploadEpisodeRecording(data) => upload_episode(channel_config, data).await,
        Commands::RenderChannelXml(data) => render_xml(episode_dir, channel_config, data).await,
        Commands::CreateEpisode(data) => create_episode(episode_dir, channel_config, data).await,
    }
}

async fn create_episode(
    episode_dir: PathBuf,
    channel_config: ChannelConfig,
    data: NewEpisode,
) -> Result<(), CliError> {
    let metadata = match mp3_metadata::read_from_file(&data.file) {
        Err(e) => return Err(CliError::Mp3Error(format!("{}", e))),
        Ok(metadata) => metadata,
    };
    let duraction = metadata.duration;

    let metadata = fs::metadata(data.file)?;

    let episode = Episode {
        id: Uuid::new_v4().to_string(),
        title: data.name.clone(),
        description: "Fill me in".into(),
        summary: "Fill me in".into(),
        link: Some("Fill me in, or delete me".into()),
        released_at: chrono::Utc::now(),
        image: "http://google.com".to_owned(),
        media: EpisodeMedia {
            url: data.url,
            duration: duraction.as_secs(),
            bytes: metadata.len(),
        },
        keywords: channel_config.channel.keywords.clone(),
    };

    info!("episode {:?}", episode);

    let yaml = serde_yaml::to_string(&episode)?;

    let mut episode_file = episode_dir.clone();
    episode_file.push(format!("{}.yaml", data.name));

    fs::write(episode_file, yaml)?;

    Ok(())
}

async fn upload_episode(
    channel_config: ChannelConfig,
    data: UploadEpisodeRecording,
) -> Result<(), CliError> {
    let object_key = format!(
        "{}/artifacts/{}.mp3",
        channel_config.publishing.prefix, data.name
    );
    let file = TokioFile::open(data.file).await?;
    let size = file.metadata().await?.len();

    let upload_url = upload::upload_contents(
        file,
        size,
        channel_config.publishing.region,
        channel_config.publishing.bucket.clone(),
        object_key,
    )
    .await?;
    println!("Uploaded file {}", upload_url);

    Ok(())
}

async fn render_xml(
    episode_dir: PathBuf,
    channel_config: ChannelConfig,
    render_options: RenderOptions,
) -> Result<(), CliError> {
    let paths = fs::read_dir(episode_dir)?;
    let mut episodes: Vec<Episode> = Vec::new();

    for path in paths {
        let text = fs::read_to_string(path?.path())?;
        let episode: Episode = serde_yaml::from_str(&text)?;
        episodes.push(episode);
    }

    let rendered_podcast = xml::generate_podcast_xml(channel_config.channel, episodes)?;

    println!("{}", rendered_podcast);

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
    }

    Ok(())
}
