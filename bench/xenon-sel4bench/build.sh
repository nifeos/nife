#!/bin/sh
# Build seL4's sel4bench for x86_64 exactly as the seL4 Foundation builds its published Skylake row,
# and pack it into one UEFI file xenon can boot from a stick or over the network (milestone 127).
#
#     sh bench/xenon-sel4bench/build.sh OUTDIR
#
# Name: provisional, minted 2026-10-21 (UTC) by lane milestone/127-xenon-sel4bench-calibration,
# with the directory. calef has not ratified either.
#
# WHAT IS PINNED, AND TO WHAT. sel4.systems/performance.html says it was generated for
# sel4bench-manifest 11d0fc2c, and gives the build command per row. The Skylake rows are the
# Foundation's CI runs `PC99_64_noksw` and `PC99_MCS_64_noksw` on its machine `skylake` (an
# i7-6700): seL4/ci-actions `sel4bench/builds.yml`, `results:`. The compiler is the one the page
# names, gcc 14.2.0, because the image below is the Foundation's own build image and the page says
# so. The page's command omits RELEASE because sel4bench's easy-settings.cmake defaults it ON.
#
# The image is multi-architecture and this runs the host's own. On Apple silicon that is arm64,
# whose x86_64-linux-gnu-gcc is Debian's 14.2.0-19, the same package version as the amd64 image's
# native gcc, and on 2026-10-21 the two produced byte-identical noksw images
# (notes/benchmarks/sel4bench-on-xenon.md has the digests). The amd64 image works here too, under
# qemu-user, at about twenty times the time.
#
# WHAT IT BUILDS. Ten configurations, each a kernel and a sel4benchapp root task:
#
#   noksw               the published Default row, the page's command verbatim
#   mcs-noksw           the published MCS row, the page's command verbatim
#   ksw                 the skim window ON, CI's `PC99_64`, which the Foundation runs on the same
#                       machine and publishes only in seL4/sel4bench-results
#   <any>-noirq         the same command with -DIRQUSER=OFF
#   <noksw any>-patched the kernel with one branch replaced by a NOP (noksw-boots-anyway.py)
#
# The -noirq set exists because sel4bench's IRQ benchmark finds the HPET through ACPI, and
# libplatsupport finds ACPI only by scanning the legacy BIOS area for the root pointer, which UEFI
# firmware need not fill. Without it the benchmark falls back to the PIT, cannot get the I/O port,
# and the run never reaches its results. Leaving the IRQ app out changes only the root task: the
# build below refuses unless the kernel and every other benchmark binary, the IPC one included,
# are byte-identical to the full build's.
#
# The patched pair exists because seL4 refuses to boot a kernel without the skim window (its
# Meltdown mitigation) on a CPU whose microcode enumerates IA32_ARCH_CAPABILITIES without RDCL_NO.
# The Foundation's i7-6700 evidently does not enumerate it, or its noksw runs could not boot. A
# release kernel refuses in silence. The patched kernels are the built ones with that branch's six
# bytes replaced and every other byte the same, so nothing sel4bench times moves.
#
# OUTPUT. OUTDIR/esp/<configuration>/EFI/BOOT/BOOTX64.EFI, one per configuration, is GRUB
# (Debian's 2.12, x86_64-efi) with grub.cfg and that configuration's kernel and root task in its
# memdisk. One file each, served by `script/board-netboot --root OUTDIR/esp/<configuration>` exactly
# as nife's own image is (bench/xenon-netboot/dnsmasq.conf names that path), or copied to a stick.
# A file per configuration rather than one menu, because nobody types at xenon's GRUB: the
# configuration is chosen by what patagonia serves. OUTDIR/RECORD.txt holds every pin, digest and
# configuration value a reader needs to say what ran.
#
# BUGS
#   * Debian's GRUB is whatever the archive serves on the day; RECORD.txt names the version that
#     went in, and nothing pins it.
#   * It refuses an OUTDIR that already holds a build directory rather than deleting one; use a
#     fresh OUTDIR per build.

set -eu
[ $# -eq 1 ] || { echo "usage: sh bench/xenon-sel4bench/build.sh OUTDIR" >&2; exit 2; }
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$1"
out="$(cd "$1" && pwd)"
image="docker.io/trustworthysystems/sel4:latest"

podman pull "$image" >/dev/null
podman run --rm -v "$out":/w -v "$here":/recipe:ro "$image" bash -euc '
manifest=11d0fc2c
cd /w
mkdir -p src && cd src
if [ ! -d .repo ]; then
  repo init -q -u https://github.com/seL4/sel4bench-manifest.git
  git -C .repo/manifests checkout -q "$manifest"
fi
repo sync -q -j4 --no-manifest-update
repo manifest -r -o ../pinned.xml

build() {
  local name=$1; shift
  mkdir "build-$name" && cd "build-$name"
  ../init-build.sh "$@" > configure.log 2>&1
  ninja > ninja.log 2>&1
  cd ..
  mkdir -p "/w/images/$name"
  cp "build-$name/images/kernel-x86_64-pc99" "/w/images/$name/kernel"
  cp "build-$name/images/sel4benchapp-image-x86_64-pc99" "/w/images/$name/sel4benchapp"
  grep -E "^Kernel[A-Za-z0-9]*:|^(RELEASE|FASTPATH|HARDWARE|FAULT|ITERATIONS|MCS|PLATFORM):" \
    "build-$name/CMakeCache.txt" | sort > "/w/images/$name/config.txt"
}

published="-DFASTPATH=TRUE -DHARDWARE=TRUE -DFAULT=TRUE -DITERATIONS=5 -DPLATFORM=x86_64"
build noksw -DKernelSkimWindow=FALSE $published
build mcs-noksw -DKernelSkimWindow=FALSE -DMCS=TRUE $published
build ksw $published
for name in noksw mcs-noksw ksw; do
  case $name in ksw) skim= ;; *) skim=-DKernelSkimWindow=FALSE ;; esac
  case $name in mcs-*) mcs=-DMCS=TRUE ;; *) mcs= ;; esac
  build "$name-noirq" $skim $mcs -DIRQUSER=OFF $published
  cmp "build-$name/images/kernel-x86_64-pc99" "build-$name-noirq/images/kernel-x86_64-pc99"
  same=0
  for app in "build-$name-noirq"/apps/sel4bench/*/; do
    app=$(basename "$app")
    [ -f "build-$name-noirq/apps/sel4bench/$app/$app" ] || continue
    cmp "build-$name/apps/sel4bench/$app/$app" "build-$name-noirq/apps/sel4bench/$app/$app"
    same=$((same + 1))
  done
  [ "$same" -ge 7 ] || { echo "build: $name-noirq compared only $same benchmark binaries" >&2; exit 1; }
done
for name in noksw mcs-noksw noksw-noirq mcs-noksw-noirq; do
  mkdir -p "/w/images/$name-patched"
  python3 -I /recipe/noksw-boots-anyway.py "/w/images/$name/kernel" "/w/images/$name-patched/kernel"
  cp "/w/images/$name/sel4benchapp" "/w/images/$name/config.txt" "/w/images/$name-patched/"
done

{
  echo "sel4bench-manifest $manifest"
  cc=$(sed -n "s/^set(CMAKE_C_COMPILER \"\(.*\)\")$/\1/p" build-noksw/CMakeFiles/*/CMakeCCompiler.cmake)
  echo "compiler $cc: $($cc --version | head -1)"
  echo; cat /w/pinned.xml
  for d in /w/images/*/; do echo; echo "== $(basename "$d")"; cat "$d/config.txt"; done
} > /w/RECORD.txt
'

# Debian packages GRUB's x86_64-efi modules for amd64 only, so this step runs in the amd64 image
# whatever the host is; it is one package install and one archive, short even under qemu-user.
podman run --rm --platform linux/amd64 -v "$out":/w -v "$here":/recipe:ro "$image" bash -euc '
apt-get update -qq >/dev/null
apt-get install -y -qq --no-install-recommends grub-efi-amd64-bin grub-common >/dev/null
cd /w/images
for d in */; do
  d=${d%/}
  echo "set variant=$d" > "$d/variant.cfg"
  mkdir -p "/w/esp/$d/EFI/BOOT"
  grub-mkstandalone -O x86_64-efi -o "/w/esp/$d/EFI/BOOT/BOOTX64.EFI" \
    --modules="multiboot2 serial terminal rdmsr smbios regexp echo test" \
    "boot/grub/grub.cfg=/recipe/grub.cfg" "sel4/variant.cfg=$d/variant.cfg" \
    "sel4/kernel=$d/kernel" "sel4/sel4benchapp=$d/sel4benchapp"
done
{
  echo; echo "grub $(dpkg-query -W -f "\${Version}" grub-efi-amd64-bin)"
  echo; cd /w && sha256sum esp/*/EFI/BOOT/BOOTX64.EFI images/*/kernel images/*/sel4benchapp
} >> /w/RECORD.txt
'
echo "build: one boot file per configuration under $out/esp;"
echo "build: serve one with script/board-netboot --root $out/esp/noksw"
