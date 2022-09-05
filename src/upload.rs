use futures::TryStreamExt;
use log::info;
use pbr::{ProgressBar, Units};
use read_progress_stream::ReadProgressStream;
use rusoto_core::ByteStream;
use rusoto_s3::S3;
use rusoto_s3::{PutObjectRequest, S3Client};
use tokio::io::AsyncRead;
use tokio_util::codec::{BytesCodec, FramedRead};

pub async fn upload_contents<R>(
    read: R,
    size: u64,
    region: crate::config::Region,
    bucket: String,
    object_key: String,
) -> Result<String, crate::CliError>
where
    R: AsyncRead + Send + Sync + 'static,
{
    let reader = FramedRead::new(read, BytesCodec::new()).map_ok(|r| r.freeze());
    let endpoint = region.endpoint.clone();
    info!("file size: {}, region {:?}", size, &region);

    let client = S3Client::new(region.into());

    let mut pb = ProgressBar::new(size);
    pb.set_units(Units::Bytes);
    pb.show_speed = true;

    if let Some(name) = object_key.split("/").last() {
        pb.message(&format!("{} ", &name));
    }

    // Progress handler to be called as bytes are read
    let progress = Box::new(move |amount: u64, _| {
        pb.add(amount);
    });

    let stream = ReadProgressStream::new(reader, progress);

    let body = ByteStream::new_with_size(stream, size as usize);

    let mime = mime_guess::from_path(&object_key)
        .first()
        .map(|x| x.to_string())
        .unwrap_or_else(|| {
            if object_key.ends_with("mp3") {
                mime::MPEG.to_string()
            } else {
                mime::APPLICATION_OCTET_STREAM.to_string()
            }
        });

    let put_request = PutObjectRequest {
        bucket: bucket.clone(),
        key: object_key.clone(),
        body: Some(body),
        acl: Some("public-read".to_owned()),
        content_type: Some(mime),
        ..Default::default()
    };

    client.put_object(put_request).await?;

    Ok(format!("https://{}.{}/{}", &bucket, endpoint, &object_key))
}
