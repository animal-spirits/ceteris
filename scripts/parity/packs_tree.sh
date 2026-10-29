# Sourced by the parity scripts. `packs_tree ROOT DIR` makes DIR a checkout of the Python pack
# package as it was before its deletion: the `.py` files of `python/ceteris` from the commit
# before the one that deleted build.py, the configuration (`*.toml`) of ROOT's current tree, and
# a link to every other top-level entry of ROOT. The package finds data/cache, its configuration
# and the cargo workspace from its own path (`parents[3]`), so they must sit beside it. Run the
# Python with PYTHONPATH=DIR/python and -P.
packs_tree() {
    local root="$1" dir="$2" deleted f
    deleted="$(git -C "$root" log -1 --format=%H --diff-filter=D -- python/ceteris/packs/build.py 2>/dev/null)"
    if [[ -z "$deleted" ]] || ! git -C "$root" cat-file -e "$deleted^:python/ceteris/packs/build.py" 2>/dev/null; then
        echo "packs_tree: no commit in this history deletes python/ceteris/packs/build.py" \
            "(a shallow clone?); the Python pack package cannot be restored" >&2
        return 1
    fi
    mkdir -p "$dir"
    git -C "$root" archive "$deleted^" python/ceteris | tar -x -C "$dir"
    cp "$root"/python/ceteris/packs/*.toml "$dir/python/ceteris/packs/"
    for f in "$root"/* "$root"/.cargo; do
        [[ "$(basename "$f")" == python ]] || ln -sfn "$f" "$dir/$(basename "$f")"
    done
}
