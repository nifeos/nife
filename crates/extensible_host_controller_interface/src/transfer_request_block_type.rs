/// Normal: a data buffer on a bulk or interrupt ring.
pub const NORMAL: u8 = 1;
/// Setup Stage of a control transfer.
pub(crate) const SETUP: u8 = 2;
/// Data Stage of a control transfer.
pub const DATA: u8 = 3;
/// Status Stage of a control transfer.
pub const STATUS: u8 = 4;
/// Link: the producer ring's way back to its start.
pub(crate) const LINK: u8 = 6;
/// Enable Slot command.
pub(crate) const ENABLE_SLOT: u8 = 9;
/// Disable Slot command.
pub(crate) const DISABLE_SLOT: u8 = 10;
/// Address Device command.
pub(crate) const ADDRESS_DEVICE: u8 = 11;
/// Configure Endpoint command.
pub(crate) const CONFIGURE_ENDPOINT: u8 = 12;
/// Evaluate Context command.
pub(crate) const EVALUATE_CONTEXT: u8 = 13;
/// Reset Endpoint command: take an endpoint out of the Halted state a stall left it in.
pub(crate) const RESET_ENDPOINT: u8 = 14;
/// Set TR Dequeue Pointer command: move an endpoint's ring past what a stall abandoned.
pub(crate) const SET_TR_DEQUEUE: u8 = 16;
/// Transfer Event.
pub(crate) const TRANSFER_EVENT: u8 = 32;
/// Command Completion Event.
pub(crate) const COMMAND_COMPLETION: u8 = 33;
/// Port Status Change Event.
pub(crate) const PORT_STATUS_CHANGE: u8 = 34;
