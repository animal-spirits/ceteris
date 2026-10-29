# Sourced by the step 3 scripts. `packs_tree DIR` makes DIR a checkout of the Python pack package
# as it was before its deletion: `python/ceteris` from the commit before the one that deleted
# build.py, and a link to every other top-level entry of the checkout. The package finds
# data/cache, its configuration and the cargo workspace from its own path (`parents[3]`), so they
# must sit beside it. Run the Python with PYTHONPATH=DIR/python and -P.
packs_tree() {
    local dir="$1" deleted f
    deleted="$(git -C "$root" log -1 --format=%H --diff-filter=D -- python/ceteris/packs/build.py)"
    [[ -n "$deleted" ]] || {
        echo "packs_tree: no commit deletes python/ceteris/packs/build.py" >&2
        return 1
    }
    mkdir -p "$dir"
    git -C "$root" archive "$deleted^" python/ceteris | tar -x -C "$dir"
    for f in "$root"/* "$root"/.cargo; do
        [[ "$(basename "$f")" == python ]] || ln -sfn "$f" "$dir/$(basename "$f")"
    done
}
