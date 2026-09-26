//! Command-line option parsing.
//!
//! Port of `caca/getopt.c`: a small `getopt_long` reimplementation covering
//! short-option bundles and `--long[=arg]` options. The global `caca_optind` /
//! `caca_optarg` state of the C library becomes an idiomatic [`Getopt`] value.
//!
//! ```
//! use libcaca::getopt::{Getopt, LongOpt};
//!
//! let mut g = Getopt::new(["prog", "-a", "--output", "file"]);
//! let longopts = &[LongOpt { name: "output", has_arg: true, val: b'o' as i32 }];
//! assert_eq!(g.next("a", &[], None), b'a' as i32);
//! assert_eq!(g.next("a", longopts, None), b'o' as i32);
//! assert_eq!(g.optarg(), Some("file"));
//! ```

/// A long option description, mirroring `struct caca_option`.
#[derive(Debug, Clone, Copy)]
pub struct LongOpt {
    /// The option name without leading dashes.
    pub name: &'static str,
    /// Whether the option takes an argument.
    pub has_arg: bool,
    /// The value returned when the option is matched.
    pub val: i32,
}

#[cfg(feature = "std")]
use alloc::string::String;
use alloc::vec::Vec;

/// A `getopt_long`-style parser.
pub struct Getopt {
    args: Vec<Vec<u8>>,
    /// The index of the next argument to examine.
    pub optind: usize,
    short_pos: Option<(usize, usize)>,
    optarg: Option<Vec<u8>>,
}

impl Getopt {
    /// Build a parser over process-style arguments (including `argv[0]`).
    pub fn new<I, S>(args: I) -> Getopt
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Getopt {
            args: args
                .into_iter()
                .map(|a| a.as_ref().as_bytes().to_vec())
                .collect(),
            optind: 1,
            short_pos: None,
            optarg: None,
        }
    }

    /// The argument of the last option that took one, if any.
    pub fn optarg(&self) -> Option<&str> {
        self.optarg
            .as_deref()
            .map(|b| core::str::from_utf8(b).unwrap_or(""))
    }

    /// Parse the next option.
    ///
    /// Returns the option value, `b'?' as i32` for an unknown option, or `-1`
    /// when option parsing is finished. The optional `longindex` receives the
    /// index of the matched long option.
    ///
    /// Like the C implementation, an unknown option is reported without
    /// advancing: repeated calls keep returning `b'?'`. Increment [`Getopt::optind`]
    /// to skip a bad option.
    pub fn next(
        &mut self,
        optstring: &str,
        longopts: &[LongOpt],
        longindex: Option<&mut usize>,
    ) -> i32 {
        self.optarg = None;

        // A pending `-xyz` bundle takes precedence over `optind`.
        if let Some((idx, off)) = self.short_pos {
            if idx >= self.args.len() {
                self.short_pos = None;
                return -1;
            }
            let flag = self.args[idx].clone();
            if off >= flag.len() {
                self.short_pos = None;
                self.optind = idx + 1;
                return self.next(optstring, longopts, longindex);
            }
            return self.short_option(&flag, idx, off, optstring);
        }

        if self.optind >= self.args.len() {
            return -1;
        }
        let idx = self.optind;
        let flag = self.args[idx].clone();

        if flag.first() == Some(&b'-') && flag.get(1) != Some(&b'-') {
            // Short option, possibly the start of a bundle.
            if flag.len() < 2 {
                // A lone `-` ends option parsing.
                return -1;
            }
            return self.short_option(&flag, idx, 1, optstring);
        }

        if flag.first() == Some(&b'-') && flag.get(1) == Some(&b'-') {
            if flag.len() == 2 {
                return -1;
            }
            let rest = &flag[2..];
            for (i, opt) in longopts.iter().enumerate() {
                let name = opt.name.as_bytes();
                if rest.len() < name.len() || &rest[..name.len()] != name {
                    continue;
                }
                match rest.get(name.len()) {
                    Some(b'=') => {
                        if !opt.has_arg {
                            self.warn_unrecognized(rest);
                            return b'?' as i32;
                        }
                        if let Some(li) = longindex {
                            *li = i;
                        }
                        self.optind = idx + 1;
                        self.optarg = Some(rest[name.len() + 1..].to_vec());
                        return opt.val;
                    }
                    None => {
                        if let Some(li) = longindex {
                            *li = i;
                        }
                        self.optind = idx + 1;
                        if opt.has_arg {
                            self.optarg = self.args.get(idx + 1).cloned();
                            self.optind = idx + 2;
                        }
                        return opt.val;
                    }
                    _ => {}
                }
            }
            self.warn_unrecognized(rest);
            return b'?' as i32;
        }

        -1
    }

    /// Handle one short option at byte offset `off` within `flag`.
    fn short_option(&mut self, flag: &[u8], idx: usize, off: usize, optstring: &str) -> i32 {
        let ret = flag[off] as i32;
        let optb = optstring.as_bytes();

        let p = match optb.iter().position(|&b| b as i32 == ret) {
            None => return b'?' as i32,
            Some(p) => p,
        };
        if optb[p] == b':' {
            return b'?' as i32;
        }

        if p + 1 < optb.len() && optb[p + 1] == b':' {
            // The option takes an argument.
            if off + 1 < flag.len() {
                self.optarg = Some(flag[off + 1..].to_vec());
                self.optind = idx + 1;
            } else {
                self.optarg = self.args.get(idx + 1).cloned();
                self.optind = idx + 2;
            }
            self.short_pos = None;
            return ret;
        }

        // Plain flag; stay inside a bundle if more characters follow.
        if off + 1 < flag.len() {
            self.short_pos = Some((idx, off + 1));
        } else {
            self.short_pos = None;
            self.optind = idx + 1;
        }
        ret
    }

    fn warn_unrecognized(&self, rest: &[u8]) {
        #[cfg(feature = "std")]
        eprintln!(
            "{}: unrecognized option `--{}'",
            self.prog(),
            String::from_utf8_lossy(rest)
        );
        #[cfg(not(feature = "std"))]
        let _ = (self, rest);
    }

    #[cfg(feature = "std")]
    fn prog(&self) -> String {
        self.args
            .first()
            .map(|a| String::from_utf8_lossy(a).into_owned())
            .unwrap_or_else(|| String::from("program"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_options_and_bundles() {
        let mut g = Getopt::new(["prog", "-ab", "-c", "val", "pos"]);
        assert_eq!(g.next("abc:", &[], None), b'a' as i32);
        // `-ab` bundles `a` then continues with `b` on the next call.
        assert_eq!(g.next("abc:", &[], None), b'b' as i32);
        assert_eq!(g.next("abc:", &[], None), b'c' as i32);
        assert_eq!(g.optarg(), Some("val"));
        assert_eq!(g.next("abc:", &[], None), -1);
        assert_eq!(g.optind, 4);
    }

    #[test]
    fn attached_argument() {
        let mut g = Getopt::new(["prog", "-cval"]);
        assert_eq!(g.next("c:", &[], None), b'c' as i32);
        assert_eq!(g.optarg(), Some("val"));
    }

    #[test]
    fn long_options() {
        let longopts = &[
            LongOpt {
                name: "output",
                has_arg: true,
                val: b'o' as i32,
            },
            LongOpt {
                name: "verbose",
                has_arg: false,
                val: b'v' as i32,
            },
        ];
        let mut idx = 0usize;
        let mut g = Getopt::new(["prog", "--output=file", "--verbose", "pos"]);
        assert_eq!(g.next("", longopts, Some(&mut idx)), b'o' as i32);
        assert_eq!(idx, 0);
        assert_eq!(g.optarg(), Some("file"));
        assert_eq!(g.next("", longopts, Some(&mut idx)), b'v' as i32);
        assert_eq!(idx, 1);
        assert_eq!(g.next("", longopts, None), -1);
    }

    #[test]
    fn unknown_does_not_advance() {
        let mut g = Getopt::new(["prog", "-z", "pos"]);
        assert_eq!(g.next("a", &[], None), b'?' as i32);
        assert_eq!(g.optind, 1);
        // Repeated calls keep reporting the same unknown option.
        assert_eq!(g.next("a", &[], None), b'?' as i32);
        g.optind += 1;
        assert_eq!(g.next("a", &[], None), -1);
    }

    #[test]
    fn long_unknown_and_terminator() {
        let mut g = Getopt::new(["prog", "--bogus", "--", "pos"]);
        assert_eq!(g.next("a", &[], None), b'?' as i32);
        g.optind += 1;
        assert_eq!(g.next("a", &[], None), -1);
        assert_eq!(g.optind, 2);
    }
}
