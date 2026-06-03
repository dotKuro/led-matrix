mod dual;
mod matrix;
mod pattern;
mod server;
mod sim;
mod ws281x;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, ValueEnum};
use tokio::sync::Notify;

use crate::dual::DualMatrix;
use crate::matrix::Matrix;
use crate::sim::{FrameBroadcast, SimulatorMatrix};
use crate::ws281x::Ws281xMatrix;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Mode {
    /// Drive the physical matrix and stream a simulator preview. For the Pi.
    Dual,
    /// Run only the in-memory simulator. No hardware accessed. For local dev.
    Simulation,
}

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(long, value_enum, default_value_t = Mode::Dual)]
    mode: Mode,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let frames = FrameBroadcast::new(16);

    match args.mode {
        Mode::Dual => {
            let physical = Ws281xMatrix::new()?;
            let simulator =
                SimulatorMatrix::new(physical.width(), physical.height(), frames.clone());
            run(
                DualMatrix {
                    a: physical,
                    b: simulator,
                },
                frames,
            )
        }
        Mode::Simulation => {
            let simulator = SimulatorMatrix::new(
                ws281x::MATRIX_WIDTH,
                ws281x::MATRIX_HEIGHT,
                frames.clone(),
            );
            run(simulator, frames)
        }
    }
}

fn run<M: Matrix>(
    mut matrix: M,
    frames: FrameBroadcast,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_state: pattern::DisplayState = Arc::new(Mutex::new(None));
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_notify = Arc::new(Notify::new());

    let width = matrix.width();
    let height = matrix.height();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    rt.spawn(server::serve(
        display_state.clone(),
        frames,
        width,
        height,
        shutdown.clone(),
        shutdown_notify.clone(),
    ));

    let ctrlc_shutdown = shutdown.clone();
    let ctrlc_notify = shutdown_notify.clone();
    rt.spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        ctrlc_shutdown.store(true, Ordering::SeqCst);
        ctrlc_notify.notify_waiters();
    });

    pattern::run(&mut matrix, &display_state, &shutdown);

    matrix.clear();
    let _ = matrix.flush();

    shutdown_notify.notify_waiters();
    rt.shutdown_background();
    Ok(())
}
