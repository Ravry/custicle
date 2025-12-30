use winit::window::Window;
use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop, ControlFlow};
use winit::event::WindowEvent;
use winit::dpi::LogicalSize;

use crate::renderer::Renderer;

mod renderer;
mod helper;

#[derive(Default)]
pub struct App {
    window: Option<Window>,
    renderer: Option<Renderer>
}

impl App {
    pub fn new() {
        let event_loop = EventLoop::new().expect("failed creating event loop!");
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut app = App::default();
        event_loop.run_app(&mut app).expect("failed running app!");
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        //create window

        let attributes = Window::default_attributes()
            .with_title("application")
            .with_inner_size(LogicalSize::new(800,600));

        self.window = Some(
            event_loop.create_window(attributes)
                .expect("failed creating window!")
        );

        //create vulkan-stuff
        self.renderer = Some(
            Renderer::new(&event_loop)
        );
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("the close button was pressed; stopping");
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                self.window.as_ref().unwrap().request_redraw();
            },
            _ => ()
        }
    }
}