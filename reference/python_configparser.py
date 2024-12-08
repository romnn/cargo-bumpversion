import configparser
from pprint import pprint


def main():
    raw_config = """
        [options.packages.find]
        exclude =
            example*
            tests*
            docs*
            build

        [bumpversion:file:CHANGELOG.md]
        replace = **unreleased**
            **v{new_version}**

        [bumpversion:part:release]
        optional_value = gamma
        values =
            dev
            gamma
    """
    config = configparser.ConfigParser()
    config.read_string(raw_config)
    pprint(config.get("options.packages.find", "exclude"))

    delimiters = ("=", ":")
    comment_prefixes = (";", "#")

    config_string = """\
[Foo Bar]
foo{0[0]}bar1
[Spacey Bar]
foo {0[0]} bar2
[Spacey Bar From The Beginning]
  foo {0[0]} bar3
  baz {0[0]} qwe
[Commented Bar]
foo{0[1]} bar4 {1[1]} comment
baz{0[0]}qwe {1[0]}another one
[Long Line]
foo{0[1]} this line is much, much longer than my editor
   likes it.
[Section\\with$weird%characters[\t]
[Internationalized Stuff]
foo[bg]{0[1]} Bulgarian
foo{0[0]}Default
foo[en]{0[0]}English
foo[de]{0[0]}Deutsch
[Spaces]
key with spaces {0[1]} value
another with spaces {0[0]} splat!
[Types]
int {0[1]} 42
float {0[0]} 0.44
boolean {0[0]} NO
123 {0[1]} strange but acceptable
[This One Has A ] In It]
  forks {0[0]} spoons
""".format(delimiters, comment_prefixes)
    print(config_string)


if __name__ == "__main__":
    main()
