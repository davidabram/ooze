#!/usr/bin/env bash

plain() {
    echo 1
}

if_else() {
    local x=$1
    if [[ $x -gt 0 ]]; then
        echo 1
    elif [[ $x -lt 0 ]]; then
        echo 2
    else
        echo 3
    fi
}

loops() {
    local n=$1
    local s=0
    for (( i = 0; i < n; i++ )); do
        s=$(( s + i ))
    done
    while [[ $s -gt 10 ]]; do
        s=$(( s - 1 ))
    done
    echo "$s"
}

switch_case() {
    local x=$1
    case $x in
        1) echo 1 ;;
        2) echo 2 ;;
        *) echo 3 ;;
    esac
}

ternary() {
    local x=$1
    [[ $x -gt 0 ]] && echo 1 || echo 2
}

bool_ops() {
    local a=$1 b=$2 c=$3
    [[ $a -eq 1 && $b -eq 1 || $c -eq 1 ]] && echo "true" || echo "false"
}

try_catch() {
    local x=$1
    if ! { [[ $x -eq 1 ]] || [[ $x -eq 2 ]]; }; then
        echo "error" >&2
        return 1
    fi
    echo "$x"
}
