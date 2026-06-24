# Mutation-operator discovery fixture for Python.
#
# Mirrors the Rust/JS fixtures' intent: each function isolates an obvious
# mutation target so discovery can be asserted by compact shape (function,
# operator, original, replacement) without caring about line/column. Where two
# operators legitimately fire on the same node, the overlap is called out in a
# comment.


def swap_boolean():
    # `True` is a plain boolean literal -> swap_boolean only.
    enabled = True
    return enabled


def negate_equality(a, b):
    return a == b


def compare(a, b):
    # The `<` operator drives both comparison_boundary (< -> <=) and
    # comparison_negation (< -> >=).
    return a < b


def swap_logical(x, y):
    return x and y


def integer_zero_one():
    # integer_zero_one is default-disabled; only discovered when explicitly enabled.
    n = 0
    return n
