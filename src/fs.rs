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

use std::env::temp_dir;
use std::fs::{OpenOptions, remove_file};
use std::io;
use std::path::PathBuf;

use rand::Rng;

pub struct TempFile {
    path: PathBuf,
}

impl TempFile {
    pub fn new() -> io::Result<Self> {
        let mut rng = Rng::new();
        let mut path = None;
        for _ in 0..50 {
            let tempfile = temp_dir()
                .join(format!("file{}", rng.next()));
            if let Ok(_) = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tempfile)
            {
                path = Some(tempfile);
                break;
            }
        }
        Ok(Self {
            path: path.ok_or(io::Error::from(io::ErrorKind::AlreadyExists))?,
        })
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
    use super::TempFile;

    #[test]
    fn test_temp_dir_exists() {
        let path;
        {
            let temp_dir = TempFile::new().expect("new temp file");
            path = temp_dir.path.clone();
            assert!(path.is_file());
        }
        assert!(!path.is_file());
        assert!(!path.exists());
    }
}
