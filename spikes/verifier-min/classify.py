"""Strict adapter for the pinned Halmos 0.3.3 full JSON output."""
import json
import sys


def classify(status, data, contract, function):
    if not isinstance(data, dict) or type(data.get("exitcode")) is not int:
        return "Incomplete"
    results = data.get("test_results")
    if not isinstance(results, dict) or len(results) != 1:
        return "Incomplete"
    name, tests = next(iter(results.items()))
    if not isinstance(name, str) or not name.endswith(":" + contract) or not isinstance(tests, list) or len(tests) != 1:
        return "Incomplete"
    test = tests[0]
    if not isinstance(test, dict) or test.get("name") != function:
        return "Incomplete"
    code = test.get("exitcode")
    if type(code) is not int or type(test.get("num_models")) is not int:
        return "Incomplete"
    if status == 1 and data["exitcode"] == 1 and code == 1 and test["num_models"] > 0:
        return "Fail"
    paths = test.get("num_paths")
    if (status == 0 and data["exitcode"] == 0 and code == 0
            and type(test.get("num_bounded_loops")) is int
            and test["num_bounded_loops"] == 0 and test["num_models"] == 0
            and isinstance(paths, list) and len(paths) == 3
            and all(type(n) is int for n in paths)
            and paths[0] > 0 and paths[1] > 0 and paths[2] == 0):
        return "Pass"
    return "Incomplete"


if __name__ == "__main__":
    try:
        with open(sys.argv[2]) as source:
            data = json.load(source)
    except (OSError, ValueError):
        data = None
    print(classify(int(sys.argv[1]), data, sys.argv[3], sys.argv[4]))
