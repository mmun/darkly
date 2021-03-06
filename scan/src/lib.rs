#![feature(pattern)]
#![feature(specialization)]
#![feature(type_ascription)]
#![feature(conservative_impl_trait)]

// Questions
// Can we impl Iterator for Scanner

// TODO
//   delimited scanning
//   non-line-broken scanning

// References
// https://doc.rust-lang.org/nightly/std/fmt/index.html
// https://docs.oracle.com/javase/7/docs/api/java/util/Scanner.html
// https://en.wikipedia.org/wiki/Scanf_format_string
// https://github.com/DanielKeep/rust-scan

use std::cmp::min;
use std::io::{Read, BufReader, BufRead};
use std::str::pattern::Pattern;
use std::str::FromStr;

// TODO Serde
pub trait Deserialize {}

pub trait Scanner {
    fn expect<'a, P: Pattern<'a>>(&'a mut self, p: P) -> Result<usize, String>;

    fn has_next(&mut self) -> bool;
    // Err case is always empty string
    fn next(&mut self) -> Result<char, String>;

    // Reads until str is full or to break.
    // It is undefined behaviour for `result` to overlap in memory with the data
    // underlying the Scanner.
    fn scan_str(&mut self, result: &mut str) -> Result<usize, String>;
    fn scan_str_to<'a, P: Pattern<'a>>(&'a mut self, result: &mut str, next: P) -> Result<usize, String>;

    fn scan<T: FromStr>(&mut self) -> Result<T, String>;
    fn scan_to<'a, T: FromStr, P: Pattern<'a>>(&'a mut self, next: P) -> Result<T, String>;

    fn scan_de<T: Deserialize>(&mut self) -> Result<T, String> { unimplemented!(); }
    fn scan_de_to<'a, T: Deserialize, P: Pattern<'a>>(&'a mut self, _next: P) -> Result<T, String> { unimplemented!(); }
}

pub fn scan_str<'a>(input: &'a str) -> impl Scanner + 'a {
    LineReadScanner::new(input.as_bytes())
}

pub fn scan_stdin<'a>() -> impl Scanner + 'a {
    LineReadScanner::new(::std::io::stdin())
}

pub fn scan_file<'a>(input: &'a ::std::fs::File) -> impl Scanner + 'a {
    LineReadScanner::new(input)
}

/// Panics if we can't open the file pointed to by path.
pub fn scan_file_from_path(path: &::std::path::Path) -> impl Scanner {
    LineReadScanner::new(::std::fs::File::open(path).unwrap())
}


// Is not kept in a state of readiness - you must call advance_line to re-establish
// invariants.
// Invariants:
// cur_line.is_some() => cur_pos < cur_line.unwrap().len()
// cur_line.is_some() <=> data to read
// cur_line does not include the terminating newline
pub struct LineReadScanner<R: Read> {
    reader: BufReader<R>,
    cur_line: Option<String>,
    cur_pos: usize,
}

impl<R: Read> LineReadScanner<R> {
    pub fn new(reader: R) -> LineReadScanner<R> {
        LineReadScanner {
            reader: BufReader::new(reader),
            cur_line: None,
            cur_pos: 0,
        }
    }

    fn read_line(&mut self) {
        let mut s = String::new();
        match self.reader.read_line(&mut s) {
            Ok(n) if n == 0 => self.cur_line = None,
            Err(_) => self.cur_line = None,
            Ok(_) => {
                if &s[s.len() - 1..] == "\n" {
                    self.cur_line = Some(s[..s.len() - 1].to_owned());
                } else {
                    self.cur_line = Some(s.to_owned());                    
                }
            }
        }
        self.cur_pos = 0;
    }

    // Assures that we are in a canonical state, i.e., either we can read, or
    // self.cur_line.is_none();
    fn advance_line(&mut self) {
        loop {
            if let Some(ref line) = self.cur_line {
                if self.cur_pos < line.len() {
                    break;
                }
            }

            self.read_line();
            if self.cur_line.is_none() {
                break;
            }
        }
    }

    fn with_cur_line<'a, F, T>(&'a mut self, f: F) -> Result<T, String>
        where F: FnOnce(&'a str, &mut usize) -> Result<T, String>
    {
        self.advance_line();
        if let Some(ref line) = self.cur_line {
            f(line, &mut self.cur_pos)
        } else {
            Err(String::new())
        }
    }

    fn scan_internal<T: FromStr>(input: &str) -> Result<T, String> {
        FromStr::from_str(input).map_err(|_| input.to_owned())
    }
}

impl<R: Read> Scanner for LineReadScanner<R> {
    fn expect<'a, P: Pattern<'a>>(&'a mut self, p: P) -> Result<usize, String> {
        self.with_cur_line(|line, cur_pos| {
            let rest = &line[*cur_pos..];
            if let Some((0, s)) = rest.match_indices(p).next() {
                *cur_pos += s.len();
                Ok(s.len())
            } else {
                Err(rest.to_owned())
            }            
        })
    }

    fn has_next(&mut self) -> bool {
        self.advance_line();
        self.cur_line.is_some()
    }

    fn next(&mut self) -> Result<char, String> {
        let result = self.with_cur_line(|line, cur_pos| {
            let rest = &line[*cur_pos..];
            if let Some(c) = rest.chars().next() {
                *cur_pos += c.len_utf8();
                Ok(c)
            } else {
                Err(rest.to_owned())
            }
        });

        if result.is_err() {
            self.cur_line = None;
        }

        result
    }

    fn scan_str(&mut self, result: &mut str) -> Result<usize, String> {
        self.with_cur_line(|line, cur_pos| {
            let rest = &line[*cur_pos..];
            let end = min(result.len(), rest.len());
            copy_str(rest, result, end);
            *cur_pos += end;
            Ok(result.len())
        })
    }

    fn scan_str_to<'a, P: Pattern<'a>>(&'a mut self, result: &mut str, next: P) -> Result<usize, String> {
        self.with_cur_line(|line, cur_pos| {
            let rest = &line[*cur_pos..];
            match rest.match_indices(next).next() {
                Some((index, s)) => {
                    let end = min(result.len(), index);
                    copy_str(rest, result, end);
                    *cur_pos += index + s.len();
                }
                None => {
                    return Err(rest.to_owned());
                    // The below code gives the correct behaviour for scan_to_or_end
                    // let end = min(result.len(), rest.len());
                    // copy_str(rest, result, end);
                    // *cur_pos = line.len();
                }
            }
            Ok(result.len())
        })        
    }

    fn scan<T: FromStr>(&mut self) -> Result<T, String> {
        let result = self.with_cur_line(|line, cur_pos| {
            let rest = &line[*cur_pos..];
            LineReadScanner::<R>::scan_internal(rest)
        });
        self.cur_line = None;
        result
    }

    // TODO should panic if we run out of text before we hit `next`
    fn scan_to<'a, T: FromStr, P: Pattern<'a>>(&'a mut self, next: P) -> Result<T, String> {
        self.with_cur_line(|line, cur_pos| {
            let rest = &line[*cur_pos..];
            match rest.match_indices(next).next() {
                Some((i, s)) => {
                    *cur_pos += i + s.len();
                    LineReadScanner::<R>::scan_internal(&rest[..i])
                }
                None => {
                    Err(rest.to_owned())
                    // The below code gives the correct behaviour for scan_to_or_end
                    // *cur_pos = line.len();
                    // LineReadScanner::<R>::scan_internal(rest)
                }
            }
        })
    }
}

// `from` and `to` must not overlap.
fn copy_str(from: &str, to: &mut str, count: usize) {
    assert!(count <= to.len());
    unsafe {
        let mfrom = from.as_bytes().as_ptr();
        let mto = ::std::mem::transmute::<&mut str, &mut [u8]>(to).as_mut_ptr();
        ::std::ptr::copy_nonoverlapping(mfrom, mto, count);
    }
}


#[cfg(test)]
mod test {
    use super::{scan_str, Scanner};

    // TODO to test
    // scan and scan_to with a few non-String types.

    #[test]
    fn test_scan() {
        let mut ss = scan_str("Hello, world!");
        assert!(ss.scan_to(",").unwrap(): String == "Hello");
        assert!(ss.next().unwrap() == ' ');
        assert!(ss.scan().unwrap(): String == "world!");
    }

    #[test]
    fn test_scan_to_int() {
        let mut ss = scan_str("Hello: 42!");
        assert!(ss.scan_to(":").unwrap(): String == "Hello");
        assert!(ss.next().unwrap() == ' ');
        assert!(ss.scan_to("!").unwrap(): u32 == 42);
    }

    #[test]
    fn test_len() {
        let mut ss = scan_str("Hello, world!");
        assert!(ss.expect("Hello").unwrap() == 5);
        ss.next().unwrap();
        ss.next().unwrap();
        // TODO match world?
    }

    #[test]
    fn test_scan_str() {
        let mut ss = scan_str("Hello, world!");

        assert!(ss.next().unwrap() == 'H');
        assert!(ss.next().unwrap() == 'e');

        let mut s = "     ".to_owned();
        ss.scan_str(&mut s).unwrap();
        assert!(s == "llo, ");

        ss.scan_str_to(&mut s, "d").unwrap();
        assert!(s == "worl ");
        assert!(ss.next().is_ok());
        assert!(!ss.has_next());
    }

    #[test]
    fn test_expect() {
        let mut ss = scan_str("Hello, world!");

        ss.expect("Hello").unwrap();
        ss.expect(',').unwrap();
        ss.expect(' ').unwrap();
        assert!(ss.next() == Ok('w'));
    }
}
