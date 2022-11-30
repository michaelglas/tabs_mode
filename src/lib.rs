use std::{os::{raw::*, unix::net::UnixStream}, env::var_os, io::Read, borrow::Borrow, cell::RefCell, rc::Rc, ffi::{CString, CStr}, marker::PhantomData, fmt::Display, sync::Mutex};
use thiserror::Error;
use std::io::Write;
use serde::Deserialize;
use fragile::Fragile;
#[macro_use]
extern crate lazy_static;


use serde::de::DeserializeOwned;

mod bindings;

#[derive(Error, Debug)]
enum IpcError {
    #[error("Error while deserializing.")]
    Serde {
        #[from]
        source: serde_json::error::Error,
    },
    #[error("io error.")]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error("Could not find the ipc socket.")]
    NotFound,
    #[error("The response header was invalid.")]
    InvalidHeader,
    #[error("The payload is too big.")]
    TooBig,
}

struct IpcStream {
    connection: UnixStream,
}

trait IpcMessage {
    fn payload_type(&self) -> u32;
    fn payload(&self) -> &[u8];

    type Result: DeserializeOwned;
}

const HEADER_LENGTH: usize = 6 + 4 + 4;
const HEADER_PREFIX: &[u8] = "i3-ipc".as_bytes();

impl IpcStream {
    pub fn connect() -> Result<IpcStream, IpcError> {
        Ok(IpcStream {
            connection: UnixStream::connect(var_os("SWAYSOCK").or(var_os("I3SOCK")).ok_or_else(|| IpcError::NotFound)?)?,
        })
    }

    pub fn write<T: IpcMessage>(&mut self, message: T) -> Result<T::Result, IpcError> {
        let mut request: Vec<u8> = Vec::new();
        let payload = message.payload();

        request.extend_from_slice(HEADER_PREFIX);
        request.extend_from_slice(&u32::try_from(payload.len()).map_err(|_| IpcError::TooBig)?.to_ne_bytes());
        request.extend_from_slice(&message.payload_type().to_ne_bytes());
        request.extend_from_slice(payload);

        self.connection.write_all(&request)?;

        let mut response_header = [0 as u8; HEADER_LENGTH];

        self.connection.read_exact(&mut response_header)?;

        if &response_header[0..6] != HEADER_PREFIX {
            return Err(IpcError::InvalidHeader);
        }

        let response_payload_length = u32::from_ne_bytes(response_header[6..10].try_into().unwrap());
        let response_payload_type = u32::from_ne_bytes(response_header[10..14].try_into().unwrap());

        if response_payload_type != message.payload_type() {
            return Err(IpcError::InvalidHeader);
        }

        let mut response_payload = vec![0 as u8; response_payload_length as usize].into_boxed_slice();

        self.connection.read_exact(response_payload.as_mut())?;

        eprintln!("response = {}", std::str::from_utf8(response_payload.as_ref()).unwrap());

        return Ok(serde_json::from_slice(response_payload.as_ref())?);
    }
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
enum NodeType {
    #[serde(rename = "root")]
    Root,
    #[serde(rename = "output")]
    Output,
    #[serde(rename = "workspace")]
    Workspace,
    #[serde(rename = "con")]
    Container,
    #[serde(rename = "floating_con")]
    FloatingContainer,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
enum Border {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "pixel")]
    Pixel,
    #[serde(rename = "csd")]
    Csd,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
enum Layout {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "splith")]
    SplitH,
    #[serde(rename = "splitv")]
    SplitV,
    #[serde(rename = "stacked")]
    Stacked,
    #[serde(rename = "tabbed")]
    Tabbed,
    #[serde(rename = "output")]
    Output,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
enum Orientation {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "horizontal")]
    Horizontal,
    #[serde(rename = "vertical")]
    Vertical,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
struct Rectangle {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
#[serde(into = "u32")]
#[serde(try_from = "u32")]
enum FullscreenMode {
    None,
    Full,
    Global,
}

impl Into<u32> for FullscreenMode {
    fn into(self) -> u32 {
        match self {
            FullscreenMode::None => 0,
            FullscreenMode::Full => 1,
            FullscreenMode::Global => 2,
        }
    }
}

impl TryFrom<u32> for FullscreenMode {
    type Error = Unequal;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FullscreenMode::None),
            1 => Ok(FullscreenMode::Full),
            2 => Ok(FullscreenMode::Global),
            _ => Err(Unequal()),
        }
    }
}


#[derive(Deserialize, PartialEq, Eq, Debug)]
enum UserIdleInhibitor {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "focus")]
    Focus,
    #[serde(rename = "fullscreen")]
    Fullscreen,
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "visible")]
    Visible,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
enum ApplicationIdleInhibitor {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "enabled")]
    Enabled,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
struct IdleInhibitors {
    user: UserIdleInhibitor,
    application: ApplicationIdleInhibitor,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
struct Tree {
    id: u64,
    name: Option<String>,
    #[serde(rename = "type")]
    type_: NodeType,
    border: Border,
    current_border_width: i32,
    layout: Layout,
    orientation: Orientation,
    rect: Rectangle,
    window_rect: Rectangle,
    deco_rect: Rectangle,
    geometry: Rectangle,
    urgent: bool,
    sticky: bool,
    marks: Vec<String>,
    focused: bool,
    focus: Vec<u64>,
    nodes: Vec<Rc<Tree>>,
    floating_nodes: Vec<Rc<Tree>>,
    representation: Option<String>,
    fullscreen_mode: Option<FullscreenMode>,
    app_id: Option<String>,
    pid: Option<i32>,
    visible: Option<bool>,
    shell: Option<String>,
    inhibit_idle: Option<bool>,
    idle_inhibitors: Option<IdleInhibitors>,
    window: Option<i32>,
}

struct GetTree();

impl IpcMessage for GetTree {
    fn payload_type(&self) -> u32 {
        4
    }
    fn payload(&self) -> &[u8] {
        &[]
    }

    type Result = Tree;
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
struct Unequal();

impl Display for Unequal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Value is not equal to the constant value.")
    }
}

trait Type {
    type Type;
}

macro_rules! declare_constant {
    ($name:ident, $value:expr, $type:ty) => {
        #[derive(Deserialize, PartialEq, Eq, Debug)]
        #[serde(into = "<Self as Type>::Type")]
        #[serde(try_from = "<Self as Type>::Type")]
        struct $name();
        impl Type for $name {
            type Type = $type;
        }

        impl Into<$type> for $name {
            fn into(self) -> $type {
                $value
            }
        }

        impl TryFrom<$type> for $name {
            type Error = Unequal;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                if (value == $value) {
                    Ok($name())
                } else {
                    Err(Unequal())
                }
            }
        }
    };
}

declare_constant!(TrueConstant, true, bool);
declare_constant!(FalseConstant, false, bool);


#[derive(Deserialize, PartialEq, Eq, Debug)]
#[serde(untagged)]
enum ExecResult {
    Success {
        success: TrueConstant,
    },
    Error {
        success: FalseConstant,
        parse_error: bool,
        error: String,
    },
}

struct Exec {
    command: String,
}

impl Exec {
    fn new(command: String) -> Self {
        Exec {
            command,
        }
    }
}

impl IpcMessage for Exec {
    fn payload_type(&self) -> u32 {
        0
    }
    fn payload(&self) -> &[u8] {
        self.command.as_bytes()
    }

    type Result = Vec<ExecResult>;
}

struct SendPtr<T>(*mut T);

unsafe impl<T> Send for SendPtr<T> {}

impl<T> SendPtr<T> {
    fn get(&self) -> *mut T {
        self.0
    }

    fn new(ptr: *mut T) -> Self {
        SendPtr(ptr)
    }
}

lazy_static! {
    static ref WIDGETS: Mutex<Vec<SendPtr<bindings::widget>>> = Mutex::new(Vec::new());
}

#[no_mangle]
pub unsafe extern "C" fn init(mode: *mut bindings::mode, map: *mut bindings::map) {
    let mut stream = IpcStream::connect().unwrap();

    let mut widgets = WIDGETS.lock().unwrap();

    if widgets.len() != 0 {
        panic!("Init called twice.");
    }

    let mut last_tabbed: Option<Rc<Tree>> = None;

    let mut current_node = Rc::new(stream.write(GetTree()).unwrap());
    while let Some(next_id) = current_node.focus.get(0).cloned() {
        if current_node.layout == Layout::Tabbed {
            last_tabbed = Some(current_node.clone());
        }

        for child in &current_node.clone().nodes {
            if child.id == next_id {
                current_node = child.clone();
            }
        }
    }

    for child in &last_tabbed.expect("Did not find a focus tabbed.").nodes {
        let title = CString::new(child.name.as_ref().unwrap().clone()).unwrap();
        let action = CString::new(child.id.to_string()).unwrap();
        let mut text = [title.as_ptr() as *mut c_char];
        let mut actions = [action.as_ptr() as *mut c_char];
        widgets.push(SendPtr::new(bindings::wofi_create_widget(mode, text.as_mut_ptr(), title.as_ptr() as *mut c_char, actions.as_mut_ptr(), 1)));
    }
}

// Probably legacy
/*#[no_mangle]
pub unsafe extern "C" fn load(mode_: *mut bindings::mode) {
}*/

#[no_mangle]
pub unsafe extern "C" fn exec(cmd: *const bindings::gchar) {
    let mut stream = IpcStream::connect().unwrap();

    stream.write(Exec::new(format!("[con_id={}] focus", CStr::from_ptr(cmd).to_str().unwrap().to_owned()))).unwrap();

    std::process::exit(0);
}

#[no_mangle]
pub unsafe extern "C" fn get_widget() -> *mut bindings::widget {
    let mut widgets = WIDGETS.lock().unwrap();
    widgets.pop().unwrap_or(SendPtr::new(std::ptr::null_mut())).get()
}

#[no_mangle]
pub unsafe extern "C" fn no_entry() -> bool {
    true
}

#[no_mangle]
pub unsafe extern "C" fn get_arg_names() -> *mut *const c_char {
    Box::leak(Box::new([])).as_mut_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn get_arg_count() -> usize {
    0
}
