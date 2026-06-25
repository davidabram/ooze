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


def quantifier(items):
    # `any(...)` drives iterator_any_all (any -> all).
    return any(x.active for x in items)


def returns_boolean(flag):
    # Each boolean literal drives return_boolean and swap_boolean.
    if flag:
        return True
    return False


def string_predicate(value):
    # `value.isdigit()` drives negate_predicate_method (wrap in `not (...)`).
    return value.isdigit()


def bounds(values):
    # `min`/`max` drive min_max_swap; min_max_swap is default-disabled.
    return min(values), max(values)
