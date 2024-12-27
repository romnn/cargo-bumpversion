from pprint import pprint
import configparser

if __name__ == "__main__":
    with open("./crates/serde-ini-spanned/test-data/cfgparser.0.ini", "r") as f:
        # config = configparser.ConfigParser()
        config = configparser.RawConfigParser(
            comment_prefixes=("#", ";", "----"),
            inline_comment_prefixes=("//",),
            empty_lines_in_values=True,
            strict=False,
        )
        config.read_string(f.read())
        for section_name in [
            "global",
        ]:
            print(section_name, dict(config[section_name]))

        # for section_name in [
        #     "DEFAULT",
        #     "corruption",
        #     "yeah, sections can be indented as well",
        #     "another one!",
        #     "no values here",
        #     "tricky interpolation",
        #     "more interpolation",
        # ]:
        #     print(section_name, dict(config[section_name]))

    # boolean_config = """
    # [bumpversion]
    # current_version = 0.1.8
    # commit = True
    # tag = True
    # message = DO NOT BUMP VERSIONS WITH THIS FILE
    # """
    # config = configparser.ConfigParser()
    # config.read_string(boolean_config)
    # current_version = config.get("bumpversion", "current_version")
    # commit = config.get("bumpversion", "commit")
    # tag = config.get("bumpversion", "tag")
    # print("current_version", type(current_version), current_version)
    # print("commit", type(commit), commit)
    # print("tag", type(tag), tag)
    #
    # boolean_config = """
    # [bumpversion]
    # current_version = 0.1.8
    # commit = true
    # tag = true
    # message = DO NOT BUMP VERSIONS WITH THIS FILE
    # """
    # config = configparser.ConfigParser()
    # config.read_string(boolean_config)
    # commit = config.get("bumpversion", "commit")
    # tag = config.get("bumpversion", "tag")
    # print("commit", type(commit), commit)
    # print("tag", type(tag), tag)
