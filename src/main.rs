mod analysis;
mod cache;
mod core;
mod editor;
mod graph;
mod platform;

use cache::cache_manager::CacheManager;
use core::config;
use core::orchestrator::Orchestrator;
use core::types::*;
use graph::graph_layout::GraphLayout;
use graph::graph_view::GraphView;
use platform::renderer::Renderer;
use platform::renderer_cairo::RendererCairo;
use platform::window_x11::WindowX11;
use std::io::Read;
use std::time::Instant;

struct Args {
    error_string: String,
    build_dir: String,
    pipe_mode: bool,
    show_asm: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        error_string: String::new(),
        build_dir: ".".to_string(),
        pipe_mode: false,
        show_asm: false,
    };

    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--error" if i + 1 < argv.len() => {
                i += 1;
                args.error_string = argv[i].clone();
            }
            "--build-dir" if i + 1 < argv.len() => {
                i += 1;
                args.build_dir = argv[i].clone();
            }
            "--pipe" => args.pipe_mode = true,
            "--show-asm" => args.show_asm = true,
            _ => {}
        }
        i += 1;
    }

    args
}

fn main() {
    env_logger::init();

    let mut args = parse_args();

    // Pipe mode: read from stdin
    if args.pipe_mode {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).ok();
        args.error_string = buf;
    }

    if args.error_string.is_empty() {
        eprintln!("Usage: relay-editor --error \"<error>\" [--build-dir <dir>] [--show-asm]");
        eprintln!("       <compiler> 2>&1 | relay-editor --pipe [--build-dir <dir>]");
        std::process::exit(1);
    }

    // Initialize cache
    let mut cache = CacheManager::new();
    let cache_path = format!("{}/.relay-cache.db", args.build_dir);
    cache.open(&cache_path);

    // Build graph from error output
    let mut orchestrator = Orchestrator::new();
    orchestrator.set_build_dir(&args.build_dir);
    let mut graph = orchestrator.build_graph_from_error(&args.error_string);

    // Auto-layout
    let layout = GraphLayout::new();
    layout.auto_layout(&mut graph);

    // Create window
    let mut window = WindowX11::new();
    if !window.create(1280, 720, "Relay Graph Editor") {
        eprintln!("Failed to create X11 window");
        std::process::exit(1);
    }

    // Create renderer
    let cr = match window.create_cairo_context() {
        Some(cr) => cr,
        None => {
            eprintln!("Failed to create Cairo context");
            std::process::exit(1);
        }
    };
    let mut renderer = RendererCairo::new(cr);

    // Set up graph view
    let mut graph_view = GraphView::new();
    graph_view.set_graph(graph);

    let mut last_time = Instant::now();

    // Main loop
    loop {
        if !window.poll_events() {
            break;
        }

        // Dispatch events to graph view
        for event in window.take_mouse_events() {
            graph_view.handle_mouse(&event);
        }
        for event in window.take_key_events() {
            // Ctrl+Q: quit
            if event.pressed && event.ctrl && event.keycode == 24 {
                break;
            }
            graph_view.handle_key(&event);
        }

        // Delta time
        let now = Instant::now();
        let dt = now.duration_since(last_time).as_secs_f64() * 1000.0;
        last_time = now;

        graph_view.update(dt);

        // Render
        if let Some(cr) = window.create_cairo_context() {
            renderer.set_context(cr);
        }

        renderer.begin_frame(window.width(), window.height());
        renderer.fill_rect(
            0.0,
            0.0,
            window.width() as f64,
            window.height() as f64,
            Color::from_hex(config::BG_COLOR, 1.0),
        );
        graph_view.render(&renderer);
        renderer.end_frame();

        window.flush();

        // Cap at ~60fps
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
}
