//! **What a mapping may do**, as capability bits every format encodes into its own entry layout.
//!
//! Name: provisional (lane maintainer/split-parents-to-zero, 2026-10-13 UTC).

pub(crate) const ACCESS_WRITE: u64 = 1 << 0;

pub(crate) const ACCESS_USER: u64 = 1 << 1;

pub(crate) const ACCESS_USER_EXEC: u64 = 1 << 2;

pub(crate) const ACCESS_KERNEL_EXEC: u64 = 1 << 3;

pub(crate) const ACCESS_GLOBAL: u64 = 1 << 4;

pub(crate) const ACCESS_DEVICE: u64 = 1 << 5;

pub(crate) const ACCESS_WRITE_COMBINE: u64 = 1 << 6;

/// **The seven access bits are distinct single bits**, so every `|` in a `Flags` constructor
/// joins words that share no bit and is its own `^`. The mutation exclusions for those
/// constructors name this constant: the compiler checks what no test can.
///
/// Name: provisional, minted 2026-10-31 (UTC) by lane/954-paging-noise.
#[allow(dead_code)] // a compile-time check; nothing refers to it by name
pub(crate) const THE_ACCESS_BITS_SHARE_NO_BIT: () = {
    assert_share_no_bit!(
        ACCESS_WRITE,
        ACCESS_USER,
        ACCESS_USER_EXEC,
        ACCESS_KERNEL_EXEC,
        ACCESS_GLOBAL,
        ACCESS_DEVICE,
        ACCESS_WRITE_COMBINE
    );
};

/// What access a mapping grants: readable always, plus write / execute / user / global / device as
/// set. **Format-neutral.** There is deliberately no constructor that is both writable and
/// executable (W^X): a page that is both is how a buffer overflow becomes code execution.
///
/// # BUGS
///
/// **W^X is a property of the constructors, not of the encoders.** No `leaf_entry` refuses a
/// `Flags` with both `ACCESS_WRITE` and an exec bit; no `pub` constructor builds one and `from_access`
/// is `pub(crate)`, and `nothing_is_both_writable_and_executable` does not iterate
/// `write_combining`. So `notes/confinement-claims.md` row 11 would not survive a new in-crate
/// constructor that got it wrong; a refusal or `debug_assert!` in each encoder would hold it.
///
/// # Examples
///
/// W^X, and no user-reachable page kernel-executable, over every constructor there is:
///
/// ```
/// use paging::Flags;
///
/// for f in [
///     Flags::kernel_code(),
///     Flags::kernel_rodata(),
///     Flags::kernel_data(),
///     Flags::device(),
///     Flags::user_code(),
///     Flags::user_rodata(),
///     Flags::user_data(),
///     Flags::user_device(),
/// ] {
///     assert!(!(f.is_writable() && (f.is_kernel_executable() || f.is_user_executable())));
///     // And the privilege split: nothing user-reachable is kernel-executable, so a wild kernel
///     // jump into a user page faults instead of running user-chosen instructions in EL1.
///     if f.is_user_accessible() {
///         assert!(!f.is_kernel_executable());
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags(u64);

impl Flags {
    /// Kernel code: readable and executable by the kernel, never writable, never user.
    pub const fn kernel_code() -> Self {
        Flags(ACCESS_KERNEL_EXEC | ACCESS_GLOBAL)
    }

    /// Kernel constants: readable by the kernel, never writable, never executable by anyone.
    pub const fn kernel_rodata() -> Self {
        Flags(ACCESS_GLOBAL)
    }

    /// Kernel data, stacks, heap: read/write by the kernel, never executable by anyone.
    pub const fn kernel_data() -> Self {
        Flags(ACCESS_WRITE | ACCESS_GLOBAL)
    }

    /// MMIO for the kernel. Device-typed, read/write, and never executable.
    pub const fn device() -> Self {
        Flags(ACCESS_WRITE | ACCESS_GLOBAL | ACCESS_DEVICE)
    }

    /// **A framebuffer for the kernel: device memory whose writes may be combined.**
    ///
    /// Everything [`device`](Self::device) is, plus leave for the CPU to gather neighbouring
    /// stores into one burst: wrong for a register, and the whole difference for a screen, where
    /// strong-uncacheable costs one bus transaction per four-byte pixel. A request: `x86_64`
    /// honours it (PAT entry 1), while aarch64 and Sv39 encode it as `device()`, so
    /// [`is_write_combining`](Self::is_write_combining) does not survive a round trip there.
    ///
    /// Name: provisional (the console-scroll lane, 2026-10-04).
    pub const fn write_combining() -> Self {
        Flags(ACCESS_WRITE | ACCESS_GLOBAL | ACCESS_DEVICE | ACCESS_WRITE_COMBINE)
    }

    /// User code (milestone 7): executable by user, never by the kernel, never writable.
    ///
    /// Not executable by the kernel is not paranoia. Without it, a bug that jumps the kernel into a
    /// user page would execute *user-controlled instructions in supervisor mode*. That is a total
    /// compromise, and it is one bit (aarch64 PXN, RISC-V's U bit denying S-mode execute).
    pub const fn user_code() -> Self {
        Flags(ACCESS_USER | ACCESS_USER_EXEC)
    }

    /// User constants (milestone 7): readable by user, and **nothing else**.
    ///
    /// An ELF's `.rodata` segment is `PF_R` alone. Without this, the loader's only non-executable
    /// choice is [`user_data`](Self::user_data), which is **writable**, so we would silently grant
    /// the program more authority than its own file asked for.
    pub const fn user_rodata() -> Self {
        Flags(ACCESS_USER)
    }

    /// User data (milestone 7): read/write by user, never executable.
    pub const fn user_data() -> Self {
        Flags(ACCESS_USER | ACCESS_WRITE)
    }

    /// **User device memory (milestone 8): a driver's MMIO, at user level.**
    ///
    /// This is the flag that lets a driver leave the kernel. A userspace console server holds a
    /// mapping of the UART's registers with *these* bits, and its user-mode stores go straight to
    /// the hardware. Device-typed (so the CPU does not cache or reorder register writes), user
    /// read/write, and never executable.
    pub const fn user_device() -> Self {
        Flags(ACCESS_USER | ACCESS_WRITE | ACCESS_DEVICE)
    }

    /// **[`write_combining`](Self::write_combining) for a driver at user level**: a userspace
    /// framebuffer driver's view of its aperture (`framebuffer_driver`). Everything
    /// [`user_device`](Self::user_device) is, plus permission to gather stores, and the same
    /// caveat: only `x86_64` honours the request. Name: provisional (the screen terminal lane).
    pub const fn user_write_combining() -> Self {
        Flags(ACCESS_USER | ACCESS_WRITE | ACCESS_DEVICE | ACCESS_WRITE_COMBINE)
    }

    /// The raw capability word. Opaque outside this crate; the formats use it to encode.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Whether this mapping is writable.
    pub const fn is_writable(self) -> bool {
        self.0 & ACCESS_WRITE != 0
    }

    /// Whether this mapping is executable in kernel (supervisor) mode.
    pub const fn is_kernel_executable(self) -> bool {
        self.0 & ACCESS_KERNEL_EXEC != 0
    }

    /// Whether this mapping is executable in user mode.
    pub const fn is_user_executable(self) -> bool {
        self.0 & ACCESS_USER_EXEC != 0
    }

    /// Whether one page mapped with `self` through one leaf and `other` through another would be
    /// writable through one and executable, by either privilege, through the other.
    ///
    /// The kernel refuses a second mapping of a frame in one address space when this holds for it
    /// and any mapping the space already has of that frame (`MappingHold::record_mapping`). Every
    /// other difference passes: read-only beside read-write, read-only beside code, and two
    /// mappings with the same flags. Symmetric, and false for two read-only mappings. See
    /// notes/mapping-log.md, "No frame is writable and executable in one space".
    ///
    /// Name: provisional, minted 2026-10-31 (UTC) by lane
    /// milestone/969-no-frame-is-writable-and-executable-in-one-space.
    pub const fn makes_writable_executable_pair(self, other: Flags) -> bool {
        let self_executable = self.is_user_executable() || self.is_kernel_executable();
        let other_executable = other.is_user_executable() || other.is_kernel_executable();
        (self.is_writable() && other_executable) || (other.is_writable() && self_executable)
    }

    /// Whether user mode may access this mapping at all.
    pub const fn is_user_accessible(self) -> bool {
        self.0 & ACCESS_USER != 0
    }

    /// Device-typed memory: the CPU must not cache or reorder accesses to it.
    pub const fn is_device(self) -> bool {
        self.0 & ACCESS_DEVICE != 0
    }

    /// Device memory whose stores may be gathered and posted: see
    /// [`write_combining`](Self::write_combining). Implies [`is_device`](Self::is_device).
    /// Name: provisional (the console-scroll lane, 2026-10-04).
    pub const fn is_write_combining(self) -> bool {
        self.0 & ACCESS_WRITE_COMBINE != 0
    }

    /// Global mappings match TLB lookups under every address space; non-global ones only under
    /// their own. User mappings must be non-global (so a context switch flushes nothing); kernel
    /// mappings must be global (the high half is identical for everyone).
    pub const fn is_global(self) -> bool {
        self.0 & ACCESS_GLOBAL != 0
    }

    /// Build `Flags` from a raw capability word. Crate-internal: the formats' `leaf_flags` decoders
    /// reassemble a `Flags` this way.
    pub(crate) const fn from_access(access: u64) -> Self {
        Flags(access)
    }
}
