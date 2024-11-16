from pprint import pprint
import configparser

if __name__ == "__main__":
    boolean_config = """
    [bumpversion]
    current_version = 0.1.8
    commit = True
    tag = True
    message = DO NOT BUMP VERSIONS WITH THIS FILE
    """
    config = configparser.ConfigParser()
    config.read_string(boolean_config)
    current_version = config.get("bumpversion", "current_version")
    commit = config.get("bumpversion", "commit")
    tag = config.get("bumpversion", "tag")
    print("current_version", type(current_version), current_version)
    print("commit", type(commit), commit)
    print("tag", type(tag), tag)

    boolean_config = """
    [bumpversion]
    current_version = 0.1.8
    commit = true
    tag = true
    message = DO NOT BUMP VERSIONS WITH THIS FILE
    """
    config = configparser.ConfigParser()
    config.read_string(boolean_config)
    commit = config.get("bumpversion", "commit")
    tag = config.get("bumpversion", "tag")
    print("commit", type(commit), commit)
    print("tag", type(tag), tag)
