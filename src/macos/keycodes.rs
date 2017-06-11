use libc::uint16_t;

//https://stackoverflow.com/questions/3202629/where-can-i-find-a-list-of-mac-virtual-key-codes

/* keycodes for keys that are independent of keyboard layout*/

pub const kVK_Return: uint16_t                    = 0x24;
pub const kVK_Tab: uint16_t                       = 0x30;
pub const kVK_Space: uint16_t                     = 0x31;
pub const kVK_Delete: uint16_t                    = 0x33;
pub const kVK_Escape: uint16_t                    = 0x35;
pub const kVK_Command: uint16_t                   = 0x37;
pub const kVK_Shift: uint16_t                     = 0x38;
pub const kVK_CapsLock: uint16_t                  = 0x39;
pub const kVK_Option: uint16_t                    = 0x3A;
pub const kVK_Control: uint16_t                   = 0x3B;
pub const kVK_RightShift: uint16_t                = 0x3C;
pub const kVK_RightOption: uint16_t               = 0x3D;
pub const kVK_RightControl: uint16_t              = 0x3E;
pub const kVK_Function: uint16_t                  = 0x3F;
pub const kVK_F17: uint16_t                       = 0x40;
pub const kVK_VolumeUp: uint16_t                  = 0x48;
pub const kVK_VolumeDown: uint16_t                = 0x49;
pub const kVK_Mute: uint16_t                      = 0x4A;
pub const kVK_F18: uint16_t                       = 0x4F;
pub const kVK_F19: uint16_t                       = 0x50;
pub const kVK_F20: uint16_t                       = 0x5A;
pub const kVK_F5: uint16_t                        = 0x60;
pub const kVK_F6: uint16_t                        = 0x61;
pub const kVK_F7: uint16_t                        = 0x62;
pub const kVK_F3: uint16_t                        = 0x63;
pub const kVK_F8: uint16_t                        = 0x64;
pub const kVK_F9: uint16_t                        = 0x65;
pub const kVK_F11: uint16_t                       = 0x67;
pub const kVK_F13: uint16_t                       = 0x69;
pub const kVK_F16: uint16_t                       = 0x6A;
pub const kVK_F14: uint16_t                       = 0x6B;
pub const kVK_F10: uint16_t                       = 0x6D;
pub const kVK_F12: uint16_t                       = 0x6F;
pub const kVK_F15: uint16_t                       = 0x71;
pub const kVK_Help: uint16_t                      = 0x72;
pub const kVK_Home: uint16_t                      = 0x73;
pub const kVK_PageUp: uint16_t                    = 0x74;
pub const kVK_ForwardDelete: uint16_t             = 0x75;
pub const kVK_F4: uint16_t                        = 0x76;
pub const kVK_End: uint16_t                       = 0x77;
pub const kVK_F2: uint16_t                        = 0x78;
pub const kVK_PageDown: uint16_t                  = 0x79;
pub const kVK_F1: uint16_t                        = 0x7A;
pub const kVK_LeftArrow: uint16_t                 = 0x7B;
pub const kVK_RightArrow: uint16_t                = 0x7C;
pub const kVK_DownArrow: uint16_t                 = 0x7D;
pub const kVK_UpArrow: uint16_t                   = 0x7;