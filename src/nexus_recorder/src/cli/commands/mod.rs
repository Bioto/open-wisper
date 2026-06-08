//! CLI command implementations.

mod audio;
mod capture;
mod record;
mod screen;

pub use audio::{run_audio, AudioArgs};
pub use capture::{run_capture, CaptureArgs};
pub use record::{run_record, RecordArgs};
pub use screen::{run_screen, ScreenArgs};

use crate::error::Result;
use crate::recording::list_audio_devices;

/// List available audio devices.
pub async fn run_list_devices() -> Result<()> {
    println!("Available audio devices:");
    println!();
    
    match list_audio_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("  No devices found.");
                println!();
                println!("Note: On Linux, you may need to specify a device like:");
                println!("  sysdefault:CARD=<cardname>");
            } else {
                for device in devices {
                    println!("  {}", device);
                }
            }
        }
        Err(e) => {
            println!("  Error listing devices: {}", e);
        }
    }
    
    println!();
    println!("Use --device <name> when recording audio.");
    
    Ok(())
}
