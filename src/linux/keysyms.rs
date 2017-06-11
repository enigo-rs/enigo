use std::os::raw::c_uint;

// https://www.cl.cam.ac.uk/~mgk25/ucs/keysymdef.h


pub const XK_Return: c_uint = 0xFF0D;
pub const XK_Tab: c_uint = 0xff09;
pub const XK_Space: c_uint = 0x0020;
pub const XK_BackSpace: c_uint = 0xff08;
pub const XK_Escape: c_uint = 0xff1b;
pub const XK_Super_L: c_uint = 0xffeb;
pub const XK_Shift_L: c_uint = 0xFFE1;
pub const XK_Caps_Lock: c_uint = 0xffe5;
pub const XK_Alt_L: c_uint = 0xffe9;
pub const XK_Control_L: c_uint = 0xffe3;
pub const XK_Home: c_uint = 0xff50;
pub const XK_Page_Up: c_uint = 0xff55;
pub const XK_Page_Down: c_uint = 0xff56;
pub const XK_Left: c_uint = 0xff51;
pub const XK_Right: c_uint = 0xff53;
pub const XK_Down: c_uint = 0xff54;
pub const XK_Up: c_uint = 0xff52;

pub const XK_F1: c_uint = 0xffbe;
pub const XK_F2: c_uint = 0xffbf;
pub const XK_F3: c_uint = 0xffc0;
pub const XK_F4: c_uint = 0xffc1;
pub const XK_F5: c_uint = 0xffc2;
pub const XK_F6: c_uint = 0xffc3;
pub const XK_F7: c_uint = 0xffc4;
pub const XK_F8: c_uint = 0xffc5;
pub const XK_F9: c_uint = 0xffc6;
pub const XK_F10: c_uint = 0xffc7;
pub const XK_F11: c_uint = 0xffc8;
pub const XK_F12: c_uint = 0xffc9;