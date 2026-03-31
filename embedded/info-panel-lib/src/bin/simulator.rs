#[cfg(feature = "simulator-ui")]
use std::path::{Path, PathBuf};

#[cfg(feature = "simulator-ui")]
use info_panel_lib::simulator::InfoPanelSimulatorRuntime;
#[cfg(feature = "simulator-ui")]
use simulator::editor::{is_replay_file, load_replay, replay_state_at, RuntimeTraceItem};

#[cfg(feature = "simulator-ui")]
enum Mode {
    Open { path: PathBuf },
    Replay { path: PathBuf },
    ShowActions { path: PathBuf },
    RenderAction { path: PathBuf, index: usize },
}

#[cfg(feature = "simulator-ui")]
fn usage() -> String {
    "usage: info-panel-simulator <trace-or-replay.json> | --replay <replay.json> | --show-actions <replay.json> | --render-action <index> <replay.json>"
        .to_string()
}

#[cfg(feature = "simulator-ui")]
fn parse_args() -> Result<Mode, String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--replay") => {
            let path = args.next().ok_or_else(usage)?;
            if args.next().is_some() {
                return Err(usage());
            }
            Ok(Mode::Replay {
                path: PathBuf::from(path),
            })
        }
        Some("--show-actions") => {
            let path = args.next().ok_or_else(usage)?;
            if args.next().is_some() {
                return Err(usage());
            }
            Ok(Mode::ShowActions {
                path: PathBuf::from(path),
            })
        }
        Some("--render-action") => {
            let index = args
                .next()
                .ok_or_else(usage)?
                .parse::<usize>()
                .map_err(|err| format!("invalid action index: {err}"))?;
            let path = args.next().ok_or_else(usage)?;
            if args.next().is_some() {
                return Err(usage());
            }
            Ok(Mode::RenderAction {
                path: PathBuf::from(path),
                index,
            })
        }
        Some(path) => {
            if args.next().is_some() {
                return Err(usage());
            }
            let path = PathBuf::from(path);
            if path.exists() && is_replay_file(&path)? {
                Ok(Mode::Replay { path })
            } else {
                Ok(Mode::Open { path })
            }
        }
        None => Err(usage()),
    }
}

#[cfg(feature = "simulator-ui")]
fn load_runtime_replay(
    path: &Path,
) -> Result<simulator::editor::ReplayEnvelope<RuntimeTraceItem<InfoPanelSimulatorRuntime>>, String>
{
    load_replay(path)
}

#[cfg(feature = "simulator-ui")]
fn main() -> Result<(), String> {
    let runtime = InfoPanelSimulatorRuntime::new();
    match parse_args()? {
        Mode::Open { path } => simulator::ui::run_editor(&runtime, &path),
        Mode::Replay { path } => {
            let replay = load_runtime_replay(&path)?;
            simulator::ui::run_replay(&runtime, &path, replay)
        }
        Mode::ShowActions { path } => {
            let replay = load_runtime_replay(&path)?;
            for (index, command) in replay.commands.iter().enumerate() {
                println!("{}: {:?}", index + 1, command);
            }
            Ok(())
        }
        Mode::RenderAction { path, index } => {
            let replay = load_runtime_replay(&path)?;
            let state = replay_state_at(&runtime, &replay.initial_state, &replay.commands, index)?;
            let mut session = simulator::editor::EditorSession { path, state };
            let width = session.state.view.terminal_width.max(1);
            let height = session.state.view.terminal_height.max(1);
            let rendered =
                simulator::ui::render_state_to_ansi(&mut session, &runtime, width, height)?;
            println!("{rendered}");
            Ok(())
        }
    }
}

#[cfg(not(feature = "simulator-ui"))]
fn main() {}
