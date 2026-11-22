ERROR_RANGES = {
    "config": range(100, 200),
    "registry": range(200, 300),
    "group_buy": range(300, 400),
    "disputes": range(400, 500),
    "supplier_bond": range(500, 600),
    "reputation": range(600, 700),
    "smart_wallet": range(700, 800),
}


def validate_error_ranges(errors):
    """Reject duplicate or out-of-namespace contract error codes."""
    seen = set()
    for error in errors:
        contract = error["contract"]
        code = error["code"]
        key = (contract, code)
        if key in seen:
            raise ValueError(f"duplicate error code {code} in {contract}")
        seen.add(key)
        allowed = ERROR_RANGES.get(contract)
        if allowed is None or code not in allowed:
            raise ValueError(f"error code {code} is outside the {contract} namespace")
