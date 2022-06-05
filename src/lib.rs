//! This is a simple `multipart/form-data` ([RFC 7578][rfc7578]) document builder.
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use form_data_builder::FormData;
//!
//! let mut form = FormData::new(Vec::new()); // use a Vec<u8> as a writer
//! form.content_type_header(); // add this `Content-Type` header to your HTTP request
//!
//! form.write_path("ferris", "testdata/rustacean-flat-noshadow.png", "image/png")?;
//! form.write_field("cute", "yes")?;
//! form.finish(); // returns the writer
//! # Ok(())
//! # }
//! ```
//!
//! Looking for a feature-packed, asynchronous, robust, and well-tested `multipart/form-data`
//! library that validates things like content types? We hope you find one somewhere!
//!
//! [rfc7578]: https://www.rfc-editor.org/rfc/rfc7578.html

#![warn(clippy::pedantic)]

use rand::{thread_rng, RngCore};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::path::Path;
use std::time::SystemTime;

/// `multipart/form-data` document builder.
///
/// See the [module documentation][`crate`] for an example.
#[derive(Debug, Clone)]
pub struct FormData<W> {
    writer: Option<W>,
    boundary: String,
}

impl<W: Write> FormData<W> {
    /// Starts writing a `multipart/form-data` document to `writer`.
    ///
    /// ```
    /// # use form_data_builder::FormData;
    /// let mut form = FormData::new(Vec::new());
    /// ```
    ///
    /// This generates a nonce as a multipart boundary by combining the current system time with a
    /// random string.
    ///
    /// # Panics
    ///
    /// Panics if the random number generator fails or if the current system time is prior to the
    /// Unix epoch.
    pub fn new(writer: W) -> FormData<W> {
        let mut buf = [0; 24];

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time should be after the Unix epoch");
        (&mut buf[..4]).copy_from_slice(&now.subsec_nanos().to_ne_bytes());
        (&mut buf[4..12]).copy_from_slice(&now.as_secs().to_ne_bytes());
        thread_rng().fill_bytes(&mut buf[12..]);

        let boundary = format!("{:->68}", base64::encode_config(&buf, base64::URL_SAFE));

        FormData {
            writer: Some(writer),
            boundary,
        }
    }

    /// Finish the `multipart/form-data` document, returning the writer.
    ///
    /// ```
    /// # use form_data_builder::FormData;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut form = FormData::new(Vec::new());
    /// // ... things happen ...
    /// let document: Vec<u8> = form.finish()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if `finish()` has already been called or if the writer fails.
    pub fn finish(&mut self) -> Result<W> {
        let mut writer = self
            .writer
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Other, "you can only finish once"))?;
        write!(writer, "--{}--\r\n", self.boundary)?;
        Ok(writer)
    }

    fn write_header(
        &mut self,
        name: &str,
        filename: Option<&OsStr>,
        content_type: Option<&str>,
    ) -> Result<&mut W> {
        let writer = self.writer.as_mut().ok_or_else(|| {
            Error::new(
                ErrorKind::Other,
                "this method cannot be used after using `finish()`",
            )
        })?;

        write!(writer, "--{}\r\n", self.boundary)?;

        write!(writer, "Content-Disposition: form-data; name=\"{}\"", name)?;
        if let Some(filename) = filename {
            write!(writer, "; filename=\"{}\"", filename.to_string_lossy())?;
        }
        write!(writer, "\r\n")?;

        if let Some(content_type) = content_type {
            write!(writer, "Content-Type: {}\r\n", content_type)?;
        }

        write!(writer, "\r\n")?;
        Ok(writer)
    }

    /// Write a non-file field to the document.
    ///
    /// ```
    /// # use form_data_builder::FormData;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut form = FormData::new(Vec::new());
    /// form.write_field("butts", "lol")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if `finish()` has already been called or if the writer fails.
    pub fn write_field(&mut self, name: &str, value: &str) -> Result<()> {
        let writer = self.write_header(name, None, None)?;
        write!(writer, "{}\r\n", value)
    }

    /// Write a file field to the document, copying the data from `reader`.
    ///
    /// [RFC 7578 § 4.2](rfc7578sec4.2) advises "a name for the file SHOULD be supplied", but
    /// "isn't mandatory for cases where the file name isn't availbale or is meaningless or
    /// private".
    ///
    /// [rfc7578sec4.2]: https://www.rfc-editor.org/rfc/rfc7578.html#section-4.2
    ///
    /// ```
    /// # use form_data_builder::FormData;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut form = FormData::new(Vec::new());
    /// use std::io::Cursor;
    ///
    /// const CORRO: &[u8] = include_bytes!("../testdata/corro.svg");
    /// form.write_file("corro", Cursor::new(CORRO), None, "image/svg+xml")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if `finish()` has already been called or if the writer fails.
    pub fn write_file<R: Read>(
        &mut self,
        name: &str,
        mut reader: R,
        filename: Option<&OsStr>,
        content_type: &str,
    ) -> Result<()> {
        let writer = self.write_header(name, filename, Some(content_type))?;
        std::io::copy(&mut reader, writer)?;
        write!(writer, "\r\n")
    }

    /// Write a file field to the document, opening the file at `path` and copying its data.
    ///
    /// This method detects the `filename` parameter from the `path`. To avoid this, use
    /// [`FormData::write_file`].
    ///
    /// ```
    /// # use form_data_builder::FormData;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut form = FormData::new(Vec::new());
    /// form.write_path("corro", "testdata/corro.svg", "image/svg+xml")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if `finish()` has already been called or if the writer fails.
    pub fn write_path<P: AsRef<Path>>(
        &mut self,
        name: &str,
        path: P,
        content_type: &str,
    ) -> Result<()> {
        self.write_file(
            name,
            &mut File::open(path.as_ref())?,
            path.as_ref().file_name(),
            content_type,
        )
    }

    /// Returns the value of the `Content-Type` header that corresponds with the document.
    ///
    /// ```
    /// # use form_data_builder::FormData;
    /// # struct Request;
    /// # impl Request {
    /// #     fn with_header(&mut self, key: &str, value: String) {}
    /// # }
    /// # let mut request = Request;
    /// # let mut form = FormData::new(Vec::new());
    /// // your HTTP client's API may vary
    /// request.with_header("Content-Type", form.content_type_header());
    /// ```
    pub fn content_type_header(&self) -> String {
        format!("multipart/form-data; boundary={}", self.boundary)
    }
}

#[cfg(test)]
mod tests {
    use crate::FormData;
    use std::ffi::OsString;
    use std::io::Cursor;
    use std::path::Path;

    /// This test uses a `multipart/form-data` document generated by Firefox as a test case.
    #[test]
    fn smoke_test() {
        const CORRECT: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testdata/form-data.bin"
        ));
        const CORRO: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/corro.svg"));
        const TEXT_A: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/text-a.txt"));
        const TEXT_B: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/text-b.txt"));

        let mut form = FormData::new(Vec::new());
        assert_eq!(form.boundary.len(), 68);
        assert_eq!(form.boundary[..(36)], "-".repeat(36));
        // cheat and use the boundary Firefox generated
        form.boundary = "---------------------------20598614689265574691413388431".to_owned();

        form.write_path(
            "file-a",
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("testdata")
                .join("rustacean-flat-noshadow.png"),
            "image/png",
        )
        .unwrap();

        form.write_field("text-a", TEXT_A.trim()).unwrap();

        form.write_file(
            "file-b",
            &mut Cursor::new(CORRO.as_bytes()),
            Some(&OsString::from("corro.svg")),
            "image/svg+xml",
        )
        .unwrap();

        form.write_field("text-b", TEXT_B.trim()).unwrap();

        assert_eq!(form.finish().unwrap(), CORRECT);
    }
}
