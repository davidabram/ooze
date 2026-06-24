# Mutation-operator discovery fixture for Python-specific operators.
#
# Each function isolates one Python-shaped mutation target. Targets that are not
# under test sit in assignments and the functions end in `return None`, so the
# generic operators and `none_return` stay quiet except where an overlap is
# deliberate and called out in a comment.


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
    # `len(xs) == 0` drives len_zero_boundary (== 0 -> != 0). The `==` token also
    # feeds negate_equality, and the `0` literal feeds integer_zero_one.
    flag = len(xs) == 0
    return None


def dict_default(d, k):
    # `d.get(k, 0)` drives dict_get_default_removal. The `0` literal also feeds
    # integer_zero_one.
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
