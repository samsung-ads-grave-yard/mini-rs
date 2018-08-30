extern crate core;

use core::getopts::{Options, ParsingStyle};
use core::getopts::Fail::*;

// Tests for reqopt
#[test]
fn test_reqopt() {
    let long_args = vec!("--test=20".to_string());
    let mut opts = Options::new();
    opts.reqopt("t", "test", "testing", "TEST");
    match opts.parse(&long_args) {
      Ok(ref m) => {
        assert!(m.opt_present("test"));
        assert_eq!(m.opt_str("test").unwrap(), "20");
        assert!(m.opt_present("t"));
        assert_eq!(m.opt_str("t").unwrap(), "20");
      }
      _ => { panic!("test_reqopt failed (long arg)"); }
    }
    let short_args = vec!("-t".to_string(), "20".to_string());
    match opts.parse(&short_args) {
      Ok(ref m) => {
        assert!((m.opt_present("test")));
        assert_eq!(m.opt_str("test").unwrap(), "20");
        assert!((m.opt_present("t")));
        assert_eq!(m.opt_str("t").unwrap(), "20");
      }
      _ => { panic!("test_reqopt failed (short arg)"); }
    }
}

#[test]
fn test_reqopt_missing() {
    let args = vec!("blah".to_string());
    match Options::new()
                  .reqopt("t", "test", "testing", "TEST")
                  .parse(&args) {
      Err(OptionMissing(_)) => {},
      _ => panic!()
    }
}

#[test]
fn test_reqopt_no_arg() {
    let long_args = vec!("--test".to_string());
    let mut opts = Options::new();
    opts.reqopt("t", "test", "testing", "TEST");
    match opts.parse(&long_args) {
      Err(ArgumentMissing(_)) => {},
      _ => panic!()
    }
    let short_args = vec!("-t".to_string());
    match opts.parse(&short_args) {
      Err(ArgumentMissing(_)) => {},
      _ => panic!()
    }
}

#[test]
fn test_reqopt_multi() {
    let args = vec!("--test=20".to_string(), "-t".to_string(), "30".to_string());
    match Options::new()
                  .reqopt("t", "test", "testing", "TEST")
                  .parse(&args) {
      Err(OptionDuplicated(_)) => {},
      _ => panic!()
    }
}

// Tests for optopt
#[test]
fn test_optopt() {
    let long_args = vec!("--test=20".to_string());
    let mut opts = Options::new();
    opts.optopt("t", "test", "testing", "TEST");
    match opts.parse(&long_args) {
      Ok(ref m) => {
        assert!(m.opt_present("test"));
        assert_eq!(m.opt_str("test").unwrap(), "20");
        assert!((m.opt_present("t")));
        assert_eq!(m.opt_str("t").unwrap(), "20");
      }
      _ => panic!()
    }
    let short_args = vec!("-t".to_string(), "20".to_string());
    match opts.parse(&short_args) {
      Ok(ref m) => {
        assert!((m.opt_present("test")));
        assert_eq!(m.opt_str("test").unwrap(), "20");
        assert!((m.opt_present("t")));
        assert_eq!(m.opt_str("t").unwrap(), "20");
      }
      _ => panic!()
    }
}

#[test]
fn test_optopt_missing() {
    let args = vec!("blah".to_string());
    match Options::new()
                  .optopt("t", "test", "testing", "TEST")
                  .parse(&args) {
      Ok(ref m) => {
        assert!(!m.opt_present("test"));
        assert!(!m.opt_present("t"));
      }
      _ => panic!()
    }
}

#[test]
fn test_optopt_no_arg() {
    let long_args = vec!("--test".to_string());
    let mut opts = Options::new();
    opts.optopt("t", "test", "testing", "TEST");
    match opts.parse(&long_args) {
      Err(ArgumentMissing(_)) => {},
      _ => panic!()
    }
    let short_args = vec!("-t".to_string());
    match opts.parse(&short_args) {
      Err(ArgumentMissing(_)) => {},
      _ => panic!()
    }
}

#[test]
fn test_optopt_multi() {
    let args = vec!("--test=20".to_string(), "-t".to_string(), "30".to_string());
    match Options::new()
                  .optopt("t", "test", "testing", "TEST")
                  .parse(&args) {
      Err(OptionDuplicated(_)) => {},
      _ => panic!()
    }
}

// Tests for optflag
#[test]
fn test_optflag() {
    let long_args = vec!("--test".to_string());
    let mut opts = Options::new();
    opts.optflag("t", "test", "testing");
    match opts.parse(&long_args) {
      Ok(ref m) => {
        assert!(m.opt_present("test"));
        assert!(m.opt_present("t"));
      }
      _ => panic!()
    }
    let short_args = vec!("-t".to_string());
    match opts.parse(&short_args) {
      Ok(ref m) => {
        assert!(m.opt_present("test"));
        assert!(m.opt_present("t"));
      }
      _ => panic!()
    }
}

#[test]
fn test_optflag_missing() {
    let args = vec!("blah".to_string());
    match Options::new()
                  .optflag("t", "test", "testing")
                  .parse(&args) {
      Ok(ref m) => {
        assert!(!m.opt_present("test"));
        assert!(!m.opt_present("t"));
      }
      _ => panic!()
    }
}

#[test]
fn test_opt_end() {
    let args = vec!["--".to_owned(), "-t".to_owned()];
    match Options::new()
                  .optflag("t", "test", "testing")
                  .parse(&args) {
      Ok(ref m) => {
        assert!(!m.opt_present("test"));
        assert!(!m.opt_present("t"));
        assert_eq!(m.free.len(), 1);
        assert_eq!(m.free[0], "-t");
      }
      _ => panic!()
    }
}

#[test]
fn test_opt_only_end() {
    let args = vec!["--".to_owned()];
    match Options::new()
                  .optflag("t", "test", "testing")
                  .parse(&args) {
      Ok(ref m) => {
        assert!(!m.opt_present("test"));
        assert!(!m.opt_present("t"));
        assert_eq!(m.free.len(), 0);
      }
      _ => panic!()
    }
}

#[test]
fn test_optflag_long_arg() {
    let args = vec!("--test=20".to_string());
    match Options::new()
                  .optflag("t", "test", "testing")
                  .parse(&args) {
      Err(UnexpectedArgument(_)) => {},
      _ => panic!()
    }
}

#[test]
fn test_optflag_multi() {
    let args = vec!("--test".to_string(), "-t".to_string());
    match Options::new()
                  .optflag("t", "test", "testing")
                  .parse(&args) {
      Err(OptionDuplicated(_)) => {},
      _ => panic!()
    }
}

#[test]
fn test_optflag_short_arg() {
    let args = vec!("-t".to_string(), "20".to_string());
    match Options::new()
                  .optflag("t", "test", "testing")
                  .parse(&args) {
      Ok(ref m) => {
        // The next variable after the flag is just a free argument

        assert!(m.free[0] == "20");
      }
      _ => panic!()
    }
}

// Tests for optflagmulti
#[test]
fn test_optflagmulti_short1() {
    let args = vec!("-v".to_string());
    match Options::new()
                  .optflagmulti("v", "verbose", "verbosity")
                  .parse(&args) {
      Ok(ref m) => {
        assert_eq!(m.opt_count("v"), 1);
      }
      _ => panic!()
    }
}

#[test]
fn test_optflagmulti_short2a() {
    let args = vec!("-v".to_string(), "-v".to_string());
    match Options::new()
                  .optflagmulti("v", "verbose", "verbosity")
                  .parse(&args) {
      Ok(ref m) => {
        assert_eq!(m.opt_count("v"), 2);
      }
      _ => panic!()
    }
}

#[test]
fn test_optflagmulti_short2b() {
    let args = vec!("-vv".to_string());
    match Options::new()
                  .optflagmulti("v", "verbose", "verbosity")
                  .parse(&args) {
      Ok(ref m) => {
        assert_eq!(m.opt_count("v"), 2);
      }
      _ => panic!()
    }
}

#[test]
fn test_optflagmulti_long1() {
    let args = vec!("--verbose".to_string());
    match Options::new()
                  .optflagmulti("v", "verbose", "verbosity")
                  .parse(&args) {
      Ok(ref m) => {
        assert_eq!(m.opt_count("verbose"), 1);
      }
      _ => panic!()
    }
}

#[test]
fn test_optflagmulti_long2() {
    let args = vec!("--verbose".to_string(), "--verbose".to_string());
    match Options::new()
                  .optflagmulti("v", "verbose", "verbosity")
                  .parse(&args) {
      Ok(ref m) => {
        assert_eq!(m.opt_count("verbose"), 2);
      }
      _ => panic!()
    }
}

#[test]
fn test_optflagmulti_mix() {
    let args = vec!("--verbose".to_string(), "-v".to_string(),
                    "-vv".to_string(), "verbose".to_string());
    match Options::new()
                  .optflagmulti("v", "verbose", "verbosity")
                  .parse(&args) {
      Ok(ref m) => {
        assert_eq!(m.opt_count("verbose"), 4);
        assert_eq!(m.opt_count("v"), 4);
      }
      _ => panic!()
    }
}

// Tests for optflagopt
#[test]
fn test_optflagopt() {
    let long_args = vec!("--test".to_string());
    let mut opts = Options::new();
    opts.optflagopt("t", "test", "testing", "ARG");
    match opts.parse(&long_args) {
      Ok(ref m) => {
        assert!(m.opt_present("test"));
        assert!(m.opt_present("t"));
      }
      _ => panic!()
    }
    let short_args = vec!("-t".to_string());
    match opts.parse(&short_args) {
      Ok(ref m) => {
        assert!(m.opt_present("test"));
        assert!(m.opt_present("t"));
      }
      _ => panic!()
    }
    let short_args = vec!("-t".to_string(), "x".to_string());
    match opts.parse(&short_args) {
        Ok(ref m) => {
            assert_eq!(m.opt_str("t").unwrap(), "x");
            assert_eq!(m.opt_str("test").unwrap(), "x");
        }
        _ => panic!()
    }
    let long_args = vec!("--test=x".to_string());
    match opts.parse(&long_args) {
        Ok(ref m) => {
            assert_eq!(m.opt_str("t").unwrap(), "x");
            assert_eq!(m.opt_str("test").unwrap(), "x");
        }
        _ => panic!()
    }
    let long_args = vec!("--test".to_string(), "x".to_string());
    match opts.parse(&long_args) {
        Ok(ref m) => {
            assert_eq!(m.opt_str("t"), None);
            assert_eq!(m.opt_str("test"), None);
        }
        _ => panic!()
    }
    let no_args: Vec<String> = vec!();
    match opts.parse(&no_args) {
      Ok(ref m) => {
        assert!(!m.opt_present("test"));
        assert!(!m.opt_present("t"));
      }
      _ => panic!()
    }
}

// Tests for optmulti
#[test]
fn test_optmulti() {
    let long_args = vec!("--test=20".to_string());
    let mut opts = Options::new();
    opts.optmulti("t", "test", "testing", "TEST");
    match opts.parse(&long_args) {
      Ok(ref m) => {
        assert!((m.opt_present("test")));
        assert_eq!(m.opt_str("test").unwrap(), "20");
        assert!((m.opt_present("t")));
        assert_eq!(m.opt_str("t").unwrap(), "20");
      }
      _ => panic!()
    }
    let short_args = vec!("-t".to_string(), "20".to_string());
    match opts.parse(&short_args) {
      Ok(ref m) => {
        assert!((m.opt_present("test")));
        assert_eq!(m.opt_str("test").unwrap(), "20");
        assert!((m.opt_present("t")));
        assert_eq!(m.opt_str("t").unwrap(), "20");
      }
      _ => panic!()
    }
}

#[test]
fn test_optmulti_missing() {
    let args = vec!("blah".to_string());
    match Options::new()
                  .optmulti("t", "test", "testing", "TEST")
                  .parse(&args) {
      Ok(ref m) => {
        assert!(!m.opt_present("test"));
        assert!(!m.opt_present("t"));
      }
      _ => panic!()
    }
}

#[test]
fn test_optmulti_no_arg() {
    let long_args = vec!("--test".to_string());
    let mut opts = Options::new();
    opts.optmulti("t", "test", "testing", "TEST");
    match opts.parse(&long_args) {
      Err(ArgumentMissing(_)) => {},
      _ => panic!()
    }
    let short_args = vec!("-t".to_string());
    match opts.parse(&short_args) {
      Err(ArgumentMissing(_)) => {},
      _ => panic!()
    }
}

#[test]
fn test_optmulti_multi() {
    let args = vec!("--test=20".to_string(), "-t".to_string(), "30".to_string());
    match Options::new()
                  .optmulti("t", "test", "testing", "TEST")
                  .parse(&args) {
      Ok(ref m) => {
          assert!(m.opt_present("test"));
          assert_eq!(m.opt_str("test").unwrap(), "20");
          assert!(m.opt_present("t"));
          assert_eq!(m.opt_str("t").unwrap(), "20");
          let pair = m.opt_strs("test");
          assert!(pair[0] == "20");
          assert!(pair[1] == "30");
      }
      _ => panic!()
    }
}

#[test]
fn test_free_argument_is_hyphen() {
    let args = vec!("-".to_string());
    match Options::new().parse(&args) {
        Ok(ref m) => {
            assert_eq!(m.free.len(), 1);
            assert_eq!(m.free[0], "-");
        }
        _ => panic!()
    }
}

#[test]
fn test_unrecognized_option() {
    let long_args = vec!("--untest".to_string());
    let mut opts = Options::new();
    opts.optmulti("t", "test", "testing", "TEST");
    match opts.parse(&long_args) {
      Err(UnrecognizedOption(_)) => {},
      _ => panic!()
    }
    let short_args = vec!("-u".to_string());
    match opts.parse(&short_args) {
      Err(UnrecognizedOption(_)) => {},
      _ => panic!()
    }
}

#[test]
fn test_combined() {
    let args =
        vec!("prog".to_string(),
             "free1".to_string(),
             "-s".to_string(),
             "20".to_string(),
             "free2".to_string(),
             "--flag".to_string(),
             "--long=30".to_string(),
             "-f".to_string(),
             "-m".to_string(),
             "40".to_string(),
             "-m".to_string(),
             "50".to_string(),
             "-n".to_string(),
             "-A B".to_string(),
             "-n".to_string(),
             "-60 70".to_string());
    match Options::new()
                  .optopt("s", "something", "something", "SOMETHING")
                  .optflag("", "flag", "a flag")
                  .reqopt("", "long", "hi", "LONG")
                  .optflag("f", "", "another flag")
                  .optmulti("m", "", "mmmmmm", "YUM")
                  .optmulti("n", "", "nothing", "NOTHING")
                  .optopt("", "notpresent", "nothing to see here", "NOPE")
                  .parse(&args) {
      Ok(ref m) => {
        assert!(m.free[0] == "prog");
        assert!(m.free[1] == "free1");
        assert_eq!(m.opt_str("s").unwrap(), "20");
        assert!(m.free[2] == "free2");
        assert!((m.opt_present("flag")));
        assert_eq!(m.opt_str("long").unwrap(), "30");
        assert!((m.opt_present("f")));
        let pair = m.opt_strs("m");
        assert!(pair[0] == "40");
        assert!(pair[1] == "50");
        let pair = m.opt_strs("n");
        assert!(pair[0] == "-A B");
        assert!(pair[1] == "-60 70");
        assert!((!m.opt_present("notpresent")));
      }
      _ => panic!()
    }
}

#[test]
fn test_mixed_stop() {
    let args =
        vec!("-a".to_string(),
             "b".to_string(),
             "-c".to_string(),
             "d".to_string());
    match Options::new()
          .parsing_style(ParsingStyle::StopAtFirstFree)
          .optflag("a", "", "")
          .optopt("c", "", "", "")
          .parse(&args) {
      Ok(ref m) => {
        println!("{}", m.opt_present("c"));
        assert!(m.opt_present("a"));
        assert!(!m.opt_present("c"));
        assert_eq!(m.free.len(), 3);
        assert_eq!(m.free[0], "b");
        assert_eq!(m.free[1], "-c");
        assert_eq!(m.free[2], "d");
      }
      _ => panic!()
    }
}

#[test]
fn test_mixed_stop_hyphen() {
    let args =
        vec!("-a".to_string(),
             "-".to_string(),
             "-c".to_string(),
             "d".to_string());
    match Options::new()
          .parsing_style(ParsingStyle::StopAtFirstFree)
          .optflag("a", "", "")
          .optopt("c", "", "", "")
          .parse(&args) {
      Ok(ref m) => {
        println!("{}", m.opt_present("c"));
        assert!(m.opt_present("a"));
        assert!(!m.opt_present("c"));
        assert_eq!(m.free.len(), 3);
        assert_eq!(m.free[0], "-");
        assert_eq!(m.free[1], "-c");
        assert_eq!(m.free[2], "d");
      }
      _ => panic!()
    }
}

#[test]
fn test_multi() {
    let mut opts = Options::new();
    opts.optopt("e", "", "encrypt", "ENCRYPT");
    opts.optopt("", "encrypt", "encrypt", "ENCRYPT");
    opts.optopt("f", "", "flag", "FLAG");

    let args_single = vec!("-e".to_string(), "foo".to_string());
    let matches_single = &match opts.parse(&args_single) {
      Ok(m) => m,
      Err(_) => panic!()
    };
    assert!(matches_single.opts_present(&["e".to_string()]));
    assert!(matches_single.opts_present(&["encrypt".to_string(), "e".to_string()]));
    assert!(matches_single.opts_present(&["e".to_string(), "encrypt".to_string()]));
    assert!(!matches_single.opts_present(&["encrypt".to_string()]));
    assert!(!matches_single.opts_present(&["thing".to_string()]));
    assert!(!matches_single.opts_present(&[]));

    assert_eq!(matches_single.opts_str(&["e".to_string()]).unwrap(), "foo");
    assert_eq!(matches_single.opts_str(&["e".to_string(), "encrypt".to_string()]).unwrap(),
               "foo");
    assert_eq!(matches_single.opts_str(&["encrypt".to_string(), "e".to_string()]).unwrap(),
               "foo");

    let args_both = vec!("-e".to_string(), "foo".to_string(), "--encrypt".to_string(),
                         "foo".to_string());
    let matches_both = &match opts.parse(&args_both) {
      Ok(m) => m,
      Err(_) => panic!()
    };
    assert!(matches_both.opts_present(&["e".to_string()]));
    assert!(matches_both.opts_present(&["encrypt".to_string()]));
    assert!(matches_both.opts_present(&["encrypt".to_string(), "e".to_string()]));
    assert!(matches_both.opts_present(&["e".to_string(), "encrypt".to_string()]));
    assert!(!matches_both.opts_present(&["f".to_string()]));
    assert!(!matches_both.opts_present(&["thing".to_string()]));
    assert!(!matches_both.opts_present(&[]));

    assert_eq!(matches_both.opts_str(&["e".to_string()]).unwrap(), "foo");
    assert_eq!(matches_both.opts_str(&["encrypt".to_string()]).unwrap(), "foo");
    assert_eq!(matches_both.opts_str(&["e".to_string(), "encrypt".to_string()]).unwrap(),
               "foo");
    assert_eq!(matches_both.opts_str(&["encrypt".to_string(), "e".to_string()]).unwrap(),
               "foo");
}

#[test]
fn test_nospace() {
    let args = vec!("-Lfoo".to_string(), "-M.".to_string());
    let matches = &match Options::new()
                                 .optmulti("L", "", "library directory", "LIB")
                                 .optmulti("M", "", "something", "MMMM")
                                 .parse(&args) {
      Ok(m) => m,
      Err(_) => panic!()
    };
    assert!(matches.opts_present(&["L".to_string()]));
    assert_eq!(matches.opts_str(&["L".to_string()]).unwrap(), "foo");
    assert!(matches.opts_present(&["M".to_string()]));
    assert_eq!(matches.opts_str(&["M".to_string()]).unwrap(), ".");

}

#[test]
fn test_nospace_conflict() {
    let args = vec!("-vvLverbose".to_string(), "-v".to_string() );
    let matches = &match Options::new()
                                 .optmulti("L", "", "library directory", "LIB")
                                 .optflagmulti("v", "verbose", "Verbose")
                                 .parse(&args) {
      Ok(m) => m,
      Err(e) => panic!( "{}", e )
    };
    assert!(matches.opts_present(&["L".to_string()]));
    assert_eq!(matches.opts_str(&["L".to_string()]).unwrap(), "verbose");
    assert!(matches.opts_present(&["v".to_string()]));
    assert_eq!(3, matches.opt_count("v"));
}


#[test]
fn test_aliases_long_and_short() {
    let args = vec!("-a".to_string(), "--apple".to_string(), "-a".to_string());

    let matches = Options::new()
                          .optflagmulti("a", "apple", "Desc")
                          .parse(&args)
                          .unwrap();
    assert_eq!(3, matches.opt_count("a"));
    assert_eq!(3, matches.opt_count("apple"));
}

#[test]
fn test_usage() {
    let mut opts = Options::new();
    opts.reqopt("b", "banana", "Desc", "VAL");
    opts.optopt("a", "012345678901234567890123456789",
                         "Desc", "VAL");
    opts.optflag("k", "kiwi", "Desc");
    opts.optflagopt("p", "", "Desc", "VAL");
    opts.optmulti("l", "", "Desc", "VAL");
    opts.optflag("", "starfruit", "Starfruit");

    let expected =
"Usage: fruits

Options:
    -b, --banana VAL    Desc
    -a, --012345678901234567890123456789 VAL
                        Desc
    -k, --kiwi          Desc
    -p [VAL]            Desc
    -l VAL              Desc
        --starfruit     Starfruit
";

    let generated_usage = opts.usage("Usage: fruits");

    assert_eq!(generated_usage, expected);
}

#[test]
fn test_usage_description_wrapping() {
    // indentation should be 24 spaces
    // lines wrap after 78: or rather descriptions wrap after 54

    let mut opts = Options::new();
    opts.optflag("k", "kiwi",
        "This is a long description which won't be wrapped..+.."); // 54
    opts.optflag("a", "apple",
        "This is a long description which _will_ be wrapped..+..");
    opts.optflag("b", "banana",
        "HereWeNeedOneSingleWordThatIsLongerThanTheWrappingLengthAndThisIsIt");

    let expected =
"Usage: fruits

Options:
    -k, --kiwi          This is a long description which won't be wrapped..+..
    -a, --apple         This is a long description which _will_ be
                        wrapped..+..
    -b, --banana        HereWeNeedOneSingleWordThatIsLongerThanTheWrappingLengthAndThisIsIt
";

    let usage = opts.usage("Usage: fruits");

    assert_eq!(usage, expected)
}

#[test]
fn test_usage_description_multibyte_handling() {
    let mut opts = Options::new();
    opts.optflag("k", "k\u{2013}w\u{2013}",
        "The word kiwi is normally spelled with two i's");
    opts.optflag("a", "apple",
        "This \u{201C}description\u{201D} has some characters that could \
confuse the line wrapping; an apple costs 0.51€ in some parts of Europe.");

    let expected =
"Usage: fruits

Options:
    -k, --k–w–          The word kiwi is normally spelled with two i's
    -a, --apple         This “description” has some characters that could
                        confuse the line wrapping; an apple costs 0.51€ in
                        some parts of Europe.
";

    let usage = opts.usage("Usage: fruits");

    assert_eq!(usage, expected)
}

#[test]
fn test_usage_short_only() {
    let mut opts = Options::new();
    opts.optopt("k", "", "Kiwi", "VAL");
    opts.optflag("s", "", "Starfruit");
    opts.optflagopt("a", "", "Apple", "TYPE");

    let expected =
"Usage: fruits

Options:
    -k VAL              Kiwi
    -s                  Starfruit
    -a [TYPE]           Apple
";

    let usage = opts.usage("Usage: fruits");
    assert_eq!(usage, expected)
}

#[test]
fn test_usage_long_only() {
    let mut opts = Options::new();
    opts.optopt("", "kiwi", "Kiwi", "VAL");
    opts.optflag("", "starfruit", "Starfruit");
    opts.optflagopt("", "apple", "Apple", "TYPE");

    let expected =
    "Usage: fruits

Options:
    --kiwi VAL          Kiwi
    --starfruit         Starfruit
    --apple [TYPE]      Apple
";

    let usage = opts.usage("Usage: fruits");
    assert_eq!(usage, expected)
}

#[test]
fn test_short_usage() {
    let mut opts = Options::new();
    opts.reqopt("b", "banana", "Desc", "VAL");
    opts.optopt("a", "012345678901234567890123456789",
                 "Desc", "VAL");
    opts.optflag("k", "kiwi", "Desc");
    opts.optflagopt("p", "", "Desc", "VAL");
    opts.optmulti("l", "", "Desc", "VAL");

    let expected = "Usage: fruits -b VAL [-a VAL] [-k] [-p [VAL]] [-l VAL]..".to_string();
    let generated_usage = opts.short_usage("fruits");

    assert_eq!(generated_usage, expected);
}
#[test]
fn test_nonexistant_opt() {
    let mut opts = Options::new();
    opts.optflag("b", "bar", "Desc");
    let args: Vec<String> = Vec::new();
    let matches = opts.parse(&args).unwrap();
    assert_eq!(matches.opt_defined("foo"), false);
    assert_eq!(matches.opt_defined("bar"), true);
}
#[test]
fn test_args_with_equals() {
    let mut opts = Options::new();
    opts.optopt("o", "one", "One", "INFO");
    opts.optopt("t", "two", "Two", "INFO");

    let args = vec!("--one".to_string(), "A=B".to_string(),
                    "--two=C=D".to_string());
    let matches = &match opts.parse(&args) {
        Ok(m) => m,
        Err(e) => panic!("{}", e)
    };
    assert_eq!(matches.opts_str(&["o".to_string()]).unwrap(), "A=B");
    assert_eq!(matches.opts_str(&["t".to_string()]).unwrap(), "C=D");
}

#[test]
#[should_panic]
fn test_long_name_too_short() {
    let mut opts = Options::new();
    opts.optflag("", "a", "Oops, long option too short");
}

#[test]
#[should_panic]
fn test_undefined_opt_present() {
    let mut opts = Options::new();
    opts.optflag("h", "help", "Description");
    let args = vec!["-h"];
    match opts.parse(args) {
        Ok(matches) => assert!(!matches.opt_present("undefined")),
        Err(e) => panic!("{}", e)
    }
}
