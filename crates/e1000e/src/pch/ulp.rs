/// `FWSM.FW_VALID`: an ME is present and running, so it owns the ULP exit.
pub(crate) const FWSM_FW_VALID: u32 = 0x0000_8000;
/// `FWSM.ULP_CFG_DONE`: set while the ME's ULP configuration is still in force.
pub(crate) const FWSM_ULP_CFG_DONE: u32 = 0x0000_0400;
/// `H2ME.ULP`: the host's ULP indication.
pub(crate) const H2ME_ULP: u32 = 0x0000_0800;
/// `H2ME.ENFORCE_SETTINGS`: ask the ME to apply the host's request.
pub(crate) const H2ME_ENFORCE_SETTINGS: u32 = 0x0000_1000;
/// How long to wait for the ME, in milliseconds: FreeBSD's 250 polls of 10 ms.
pub(crate) const WAIT_MS: u64 = 2500;

/// The `H2ME` value that asks the ME to take the PHY out of ULP (the forced path).
pub const fn request(h2me: u32) -> u32 {
    (h2me & !H2ME_ULP) | H2ME_ENFORCE_SETTINGS
}
/// The `H2ME` value once the ME has answered.
pub const fn release(h2me: u32) -> u32 {
    h2me & !H2ME_ENFORCE_SETTINGS
}
/// Is the ME's ULP configuration finished?
pub const fn done(fwsm: u32) -> bool {
    fwsm & FWSM_ULP_CFG_DONE == 0
}
/// Does this machine have an ME to ask? When it does not, ULP exit is the PHY-register
/// sequence in [`super::sequence`].
pub(crate) const fn has_management_engine(fwsm: u32) -> bool {
    fwsm & FWSM_FW_VALID != 0
}
