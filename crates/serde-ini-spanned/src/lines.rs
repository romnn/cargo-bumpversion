/// An iterator over the lines of an instance of `BufRead`.
#[derive(Debug)]
pub struct Lines<B> {
    offset: usize,
    buf: B,
}

impl<B> Lines<B> {
    pub fn new(buf: B) -> Self {
        Self { buf, offset: 0 }
    }
}

impl<B: std::io::BufRead> Iterator for Lines<B> {
    type Item = Result<(usize, String), std::io::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = String::new();
        match self.buf.read_line(&mut buf) {
            Ok(0) => None,
            Ok(n) => {
                let offset = self.offset;
                self.offset += n;
                if buf.ends_with('\n') {
                    buf.pop();
                    if buf.ends_with('\r') {
                        buf.pop();
                    }
                }
                Some(Ok((offset, buf)))
            }
            Err(e) => Some(Err(e)),
        }
    }
}
