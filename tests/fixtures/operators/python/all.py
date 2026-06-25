# Snapshot fixture: one tiny example for every Python mutation operator.
#
# The companion `expected.json` pins the discovered mutants by stable fields
# only (language, operator, implementation, function, original, replacement,
# line). Unstable fields — absolute paths, byte offsets, the path-qualified id,
# and any test-runner output — are intentionally not snapshotted. The goal is a
# regression guard: every Python operator must keep firing as the engine is
# refactored. Keep each function minimal and unambiguous; where two operators
# legitimately fire on the same node, that overlap is noted in a comment.


def swap_boolean():
    # Plain boolean literal (not in return position) -> swap_boolean only.
    enabled = True
    return enabled


def negate_equality(a, b):
    return a == b


def compare(a, b):
    # The `<` token drives comparison_boundary (< -> <=) and comparison_negation
    # (< -> >=). The whole `a < b` return value also feeds none_return.
    return a < b


def swap_logical(x, y):
    return x and y


def integer_zero_one():
    # integer_zero_one is default-disabled; only discovered when explicitly enabled.
    n = 0
    return n


def is_none(a):
    flag = a is None
    return None


def membership(a, b):
    flag = a in b
    return None


def truthiness(x):
    if x:
        do_work()
    return None


def len_boundary(xs):
    # `len(xs) == 0` drives len_zero_boundary; `==` feeds negate_equality and the
    # `0` literal feeds integer_zero_one.
    flag = len(xs) == 0
    return None


def dict_default(d, k):
    # `d.get(k, 0)` drives dict_get_default_removal; the `0` feeds integer_zero_one.
    value = d.get(k, 0)
    return None


def comprehension(xs):
    items = [x for x in xs if keep(x)]
    return None


def none_return(value):
    return value


def empty_list():
    items = [a, b]
    return None


def quantifier(items):
    # `any(...)` drives iterator_any_all; the returned call also feeds none_return.
    return any(x.active for x in items)


def returns_boolean(flag):
    # Each boolean literal drives return_boolean and swap_boolean; both also feed
    # none_return, and the `if flag:` condition feeds truthiness_negation.
    if flag:
        return True
    return False


def string_predicate(value):
    # `value.isdigit()` drives negate_predicate_method; the call also feeds none_return.
    return value.isdigit()


def bounds(values):
    # `min`/`max` drive min_max_swap (default-disabled); the returned tuple feeds none_return.
    return min(values), max(values)
