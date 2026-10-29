"""One cross-platform invocation: every candidate contract failure is fatal."""
import unittest
from unittest.mock import Mock

MODULES = (
    "test_package_native_qualification",
    "test_qualify_cli_helper",
    "test_package_jxl_integer_contract",
    "test_integer_qualification_flow",
)


def load_tests(loader, tests, pattern):
    suite = unittest.TestSuite([tests])
    for name in MODULES:
        selected = loader.loadTestsFromName(name)
        if selected.countTestCases() == 0:
            raise RuntimeError(f"Candidate contract suite is empty: {name}")
        suite.addTests(selected)
    return suite


class ContractSuiteTests(unittest.TestCase):
    def test_earlier_failure_is_not_masked_by_later_successes(self):
        class Fails(unittest.TestCase):
            def runTest(self):
                self.fail("expected first-module failure")
        class Passes(unittest.TestCase):
            def runTest(self):
                pass
        loader = Mock()
        loader.loadTestsFromName.side_effect = [unittest.TestSuite([Fails()])] + [
            unittest.TestSuite([Passes()]) for _ in MODULES[1:]
        ]
        result = unittest.TestResult()
        load_tests(loader, unittest.TestSuite(), None).run(result)
        self.assertEqual(result.testsRun, len(MODULES))
        self.assertEqual(len(result.failures), 1)
        self.assertFalse(result.wasSuccessful())

    def test_empty_contract_module_refuses_qualification(self):
        loader = Mock()
        loader.loadTestsFromName.return_value = unittest.TestSuite()
        with self.assertRaisesRegex(RuntimeError, "suite is empty"):
            load_tests(loader, unittest.TestSuite(), None)
