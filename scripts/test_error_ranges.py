import unittest

from error_ranges import validate_error_ranges


class ErrorRangeTests(unittest.TestCase):
    def test_current_style_codes_are_accepted(self):
        validate_error_ranges([
            {"contract": "config", "code": 100},
            {"contract": "group_buy", "code": 343},
            {"contract": "smart_wallet", "code": 704},
        ])

    def test_duplicate_code_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "duplicate"):
            validate_error_ranges([
                {"contract": "config", "code": 100},
                {"contract": "config", "code": 100},
            ])

    def test_out_of_range_code_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "outside"):
            validate_error_ranges([{"contract": "disputes", "code": 399}])


if __name__ == "__main__":
    unittest.main()
