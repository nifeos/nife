#!/bin/sh
# Make /dev/kvm usable by the runner user, and say what it is.
#
#     helpers/open-kvm.sh
#
# On an owned runner's container /dev/kvm is already writable and there is no /etc/udev/rules.d, so
# the udev recipe below would fail on `sudo tee`. A writable device ends the step at once. On a
# hosted runner the device is root's, and GitHub's documented recipe hands it to every user. Every
# job that wants KVM runs this one script, and the owned canary's dry run runs it too, so the dry run
# cannot miss what a real job meets. KVM_DEVICE names another device path for helpers/open-kvm-selftest.sh.
#
# Name: provisional, minted by lane/owned-runner-flip-safe.

set -eu

device=${KVM_DEVICE:-/dev/kvm}

if [ -w "$device" ]; then
    ls -l "$device"
    exit 0
fi

echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' | sudo tee /etc/udev/rules.d/99-kvm4all.rules
sudo udevadm control --reload-rules
sudo udevadm trigger --name-match=kvm
ls -l "$device"
