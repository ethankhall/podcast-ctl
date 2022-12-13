use crate::config::*;
use chrono::Utc;
use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::writer::Writer;
use std::io::Cursor;

pub fn generate_podcast_xml(
    channel_details: ChannelDetails,
    episodes: Vec<Episode>,
) -> Result<String, crate::CliError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 4);

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .ok();

    writer
        .create_element("rss")
        .with_attribute(("xmlns:itunes", "http://www.itunes.com/dtds/podcast-1.0.dtd"))
        .with_attribute(("xmlns:content", "http://purl.org/rss/1.0/modules/content/"))
        .with_attribute(("version", "2.0"))
        .write_inner_content(|writer| {
            writer
                .create_element("channel")
                .write_inner_content(|writer| {
                    add_text_element(writer, "title", &channel_details.title);
                    add_text_element(
                        writer,
                        "description",
                        &comrak::markdown_to_html(&channel_details.title, &Default::default()),
                    );
                    if let Some(link) = &channel_details.link {
                        add_text_element(writer, "link", &link);
                    }
                    add_text_element(writer, "language", "en-us");
                    add_text_element(writer, "copyright", "Copyright 2022");
                    add_text_element(
                        writer,
                        "lastBuildDate",
                        &format!("{}", Utc::now().format("%a, %d %b %Y %T %z")),
                    );
                    add_text_element(
                        writer,
                        "pubDate",
                        &format!("{}", Utc::now().format("%a, %d %b %Y %T %z")),
                    );
                    add_text_element(writer, "docs", "http://blogs.law.harvard.edu/tech/rss");
                    add_text_element(writer, "webMaster", &channel_details.owner.email);
                    add_text_element(writer, "itunes:type", "Serial");

                    add_text_element(writer, "itunes:author", &channel_details.owner.email);
                    add_text_element(
                        writer,
                        "itunes:subtitle",
                        &channel_details.subtitle
                    );
                    add_text_element(
                        writer,
                        "itunes:summary",
                        &comrak::markdown_to_html(&channel_details.summary, &Default::default()),
                    );

                    writer
                        .create_element("itunes:owner")
                        .write_inner_content(|writer| {
                            add_text_element(writer, "itunes:name", &channel_details.owner.name);
                            add_text_element(writer, "itunes:email", &channel_details.owner.email);
                            Ok(())
                        })
                        .ok();

                    add_text_element(
                        writer,
                        "itunes:explicit",
                        if channel_details.explicit {
                            "Yes"
                        } else {
                            "No"
                        },
                    );

                    let image_url: &str = &channel_details.image;
                    writer
                        .create_element("itunes:image").with_attribute(("href", image_url)).write_empty().ok();
                    writer
                        .create_element("itunes:category").with_attribute(("text", "Fiction")).write_empty().ok();

                    for episode in &episodes {
                        episode.add_object(writer);
                    }

                    Ok(())
                })
                .ok();
            Ok(())
        })
        .ok();

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

fn add_text_element<W>(writer: &mut Writer<W>, key: &str, value: &str)
where
    W: std::io::Write,
{
    writer
        .create_element(key)
        .write_text_content(BytesText::new(value))
        .ok();
}

trait XmlOutput {
    fn add_object<W>(&self, writer: &mut Writer<W>)
    where
        W: std::io::Write;
}

impl XmlOutput for Episode {
    fn add_object<W>(&self, writer: &mut Writer<W>)
    where
        W: std::io::Write,
    {
        writer
            .create_element("item")
            .write_inner_content(|writer| {
                add_text_element(writer, "title", &self.title);
                add_text_element(writer, "itunes:subtitle", &self.summary);
                if let Some(link) = &self.link {
                    add_text_element(writer, "link", &link);
                }
                add_text_element(writer, "guid", &self.id);
                let url: &str = &self.media.url;
                let length: &str = &format!("{}", self.media.bytes);
                writer
                    .create_element("enclosure")
                    .with_attribute(("url", url))
                    .with_attribute(("length", length))
                    .with_attribute(("type", "audio/mpeg"))
                    .write_empty()
                    .ok();
                add_text_element(
                    writer,
                    "pubDate",
                    &format!("{}", self.released_at.format("%a, %d %b %Y %T %z")),
                );
                add_text_element(
                    writer,
                    "description",
                    &comrak::markdown_to_html(&self.description, &Default::default()),
                );
                add_text_element(
                    writer,
                    "itunes:summary",
                    &comrak::markdown_to_html(&self.description, &Default::default()),
                );
                add_text_element(
                    writer,
                    "itunes:duration",
                    &format!("{}", self.media.duration),
                );

                add_text_element(
                    writer,
                    "itunes:season",
                    &format!("{}", self.season),
                );
                add_text_element(
                    writer,
                    "itunes:episode",
                    &format!("{}", self.episode_number),
                );

                let image: &str = &self.image;
                writer
                    .create_element("itunes:image").with_attribute(("href", image)).write_empty().ok();
                add_text_element(writer, "itunes:title", &self.title);
                Ok(())
            })
            .ok();
    }
}
