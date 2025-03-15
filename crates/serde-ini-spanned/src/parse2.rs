#[derive(pest_derive::Parser)]
#[grammar = "./ini.pest"]
pub struct IniParser;

#[pest_test_gen::pest_tests(
    crate::parse2::IniParser,
    crate::parse2::Rule,
    "document",
    // subdir = "",
    dir = "tests/pest",
    strict = true,
    recursive = true,
    lazy_static = true
)]
#[cfg(test)]
mod pest_tests {}

#[cfg(test)]
mod tests {
    use super::{IniParser, Rule};
    use color_eyre::eyre;
    use pest::Parser;
    use similar_asserts::assert_eq as sim_assert_eq;
    use std::collections::HashSet;

    use pest_test::PestTester;

    static TESTER: std::sync::LazyLock<pest_test::PestTester<Rule, IniParser>> =
        std::sync::LazyLock::new(|| {
            let skip_rules = HashSet::new();
            let test_dir = pest_test::default_test_dir();
            dbg!(&test_dir);
            pest_test::PestTester::new(test_dir, "txt", Rule::document, skip_rules)
        });

    // lazy_static::lazy_static! {
    //   static ref TESTER: pest_test::PestTester<Rule, IniParser> =
    //     // pest_test::PestTester::from_defaults(Rule::document, HashSet::new());
    //     pest_test::PestTester::new(pest_test::default_test_dir(), ".txt", Rule::document, skip_rules)
    // }

    // #[test]
    // fn test_one_section() -> Result<(), pest_test::TestError<Rule>> {
    //     (*TESTER).evaluate_strict("one_section")
    // }

    #[test]
    fn test_parse_section_name() -> eyre::Result<()> {
        crate::tests::init();
        let input = "[Section]";
        let parsed = IniParser::parse(Rule::section, input)?;
        sim_assert_eq!(parsed.as_str(), input.trim());
        Ok(())
    }

    #[test]
    fn test_parse_key_value_pair() -> eyre::Result<()> {
        crate::tests::init();
        let input = "key=value";
        let parsed = IniParser::parse(Rule::key_value, input)?;
        let key_value = parsed
            .into_iter()
            .next()
            .ok_or(eyre::eyre!("missing key value"))?;
        sim_assert_eq!(key_value.as_str(), "key=value");
        Ok(())
    }

    #[test]
    fn test_parse_key() -> eyre::Result<()> {
        crate::tests::init();
        let input = "username";
        let parsed = IniParser::parse(Rule::key, input)?;
        sim_assert_eq!(parsed.as_str(), input);
        Ok(())
    }

    #[test]
    fn test_parse_value() -> eyre::Result<()> {
        crate::tests::init();
        let input = "admin";
        let parsed = IniParser::parse(Rule::value, input)?;
        sim_assert_eq!(parsed.as_str(), input);
        Ok(())
    }
}
