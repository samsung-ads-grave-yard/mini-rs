/*
 * Copyright (c) 2018 Adgear
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of
 * this software and associated documentation files (the "Software"), to deal in
 * the Software without restriction, including without limitation the rights to
 * use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
 * the Software, and to permit persons to whom the Software is furnished to do so,
 * subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
 * FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
 * COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
 * IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */

//! Provide file-system helpers like a way to create a temporary file.

use std::env::temp_dir;
use std::fs::{File, OpenOptions, remove_file};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use crate::rand::Rng;

/// A temporary file which is deleted when it goes out of scope.
pub struct TempFile {
    file: File,
    path: PathBuf,
}

impl TempFile {
    /// Creates a new temporary file with a default prefix.
    pub fn new() -> io::Result<Self> {
        Self::with_prefix("file")
    }

    /// Gets the file handle of the temporary file.
    pub fn get(&self) -> &File {
        &self.file
    }

    /// Creates a new temporary file with the specified prefix.
    pub fn with_prefix(prefix: &str) -> io::Result<Self> {
        let mut rng = Rng::new();
        for _ in 0..50 {
            let tempfile = temp_dir()
                .join(format!("{}.{}", prefix, rng.gen_int()));
            if let Ok(file) = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tempfile)
            {
                return Ok(Self {
                    file,
                    path: tempfile,
                })
            }
        }
        Err(io::Error::from(io::ErrorKind::AlreadyExists))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Err(error) = remove_file(&self.path) {
            eprintln!("Cannot remove file: {}", error);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::TempFile;

    #[test]
    fn test_temp_file_exists() {
        let path;
        {
            let temp_file = TempFile::new().expect("new temp file");
            path = temp_file.path.clone();
            assert!(path.is_file());
            writeln!(temp_file.get(), "test").expect("write");
        }
        assert!(!path.is_file());
        assert!(!path.exists());

        let path;
        {
            let temp_file = TempFile::with_prefix("mini-prefix").expect("new temp file");
            path = temp_file.path.clone();
            assert!(path.is_file());
        }
        assert!(!path.is_file());
        assert!(!path.exists());
    }
}
