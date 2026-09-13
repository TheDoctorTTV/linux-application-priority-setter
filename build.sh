#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
mode="${1:-arch}"
jobs="${BUILD_JOBS:-$(nproc)}"
package_work_dir=''

cleanup() {
    if [[ -n "$package_work_dir" && -d "$package_work_dir" ]]; then
        rm -rf -- "$package_work_dir"
    fi
}

trap cleanup EXIT

build_app() {
    cmake \
        -S "$project_root" \
        -B "$project_root/build-release" \
        -DCMAKE_BUILD_TYPE=Release
    cmake --build "$project_root/build-release" --parallel "$jobs"
    printf 'Executable: %s\n' "$project_root/build-release/application-priority-setter"
}

build_arch_package() {
    command -v makepkg >/dev/null || {
        printf 'Error: makepkg is required to build the Arch package.\n' >&2
        exit 1
    }

    local package_dir="$project_root/packaging/arch"
    local package_name='application-priority-setter'
    local package_version
    package_version="$(awk -F= '$1 == "pkgver" { print $2; exit }' "$package_dir/PKGBUILD")"
    package_work_dir="$(mktemp -d "${TMPDIR:-/tmp}/application-priority-setter-package.XXXXXX")"

    cp "$package_dir/PKGBUILD" "$package_work_dir/PKGBUILD"
    cp "$package_dir/io.github.applicationprioritysetter.desktop" "$package_work_dir/"
    tar \
        --create \
        --gzip \
        --file "$package_work_dir/$package_name-$package_version.tar.gz" \
        --directory "$project_root" \
        --exclude='./.git' \
        --exclude='./build' \
        --exclude='./build-release' \
        --exclude='./dist' \
        --exclude='./rust/target' \
        --transform "s|^\.|$package_name-$package_version|" \
        .

    (
        cd "$package_work_dir"
        MAKEFLAGS="-j$jobs" makepkg --cleanbuild --force --noconfirm
    )

    mkdir -p "$project_root/dist/arch"
    shopt -s nullglob
    local packages=("$package_work_dir"/*.pkg.tar.*)
    if (( ${#packages[@]} == 0 )); then
        printf 'Error: makepkg did not produce a package.\n' >&2
        exit 1
    fi
    cp "${packages[@]}" "$project_root/dist/arch/"
    printf 'Arch package: %s\n' "$project_root/dist/arch/${packages[0]##*/}"
}

case "$mode" in
    arch)
        build_arch_package
        ;;
    app)
        build_app
        ;;
    *)
        printf 'Usage: %s [arch|app]\n' "${0##*/}" >&2
        exit 2
        ;;
esac
