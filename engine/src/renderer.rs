use std::{borrow::Cow, ffi::{self, c_char}};
use winit::{event_loop::{ActiveEventLoop}, raw_window_handle::HasDisplayHandle};
use ash::{Entry, Instance, ext::debug_utils, vk::{self, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT, DebugUtilsMessengerEXT}};

const DEBUG_MODE_ENABLED: bool = cfg!(debug_assertions); 

//debug callback method
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = *p_callback_data;
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy()
    };

    println!(
        "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
    );

    vk::FALSE
}
//wrapper around debug information
//(used to destroy the messenger)
struct DebugCtx {debug_utils_loader: debug_utils::Instance, debug_call_back: DebugUtilsMessengerEXT }

pub struct Renderer {
    api_entry: Entry,
    instance: Instance,
    debug_ctx: Option<DebugCtx>
}
impl Renderer {
    pub fn new(event_loop: &ActiveEventLoop) -> Self {
        let api_entry = Entry::linked();
        let (instance, debug_ctx)  = Self::create_instance(&api_entry, &event_loop);

        Self {
            api_entry,
            instance,
            debug_ctx
        }
    }

    fn create_instance(api_entry: &Entry, event_loop: &ActiveEventLoop) -> (Instance, Option<DebugCtx>) {
        unsafe {
            let app_info = vk::ApplicationInfo {
                p_application_name: c"custicle".as_ptr(),
                ..Default::default()
            };

            //collecting the required extensions from the ash_window 
            let mut extension_names = 
                ash_window::enumerate_required_extensions(
                    event_loop.display_handle().expect("failed to gather display handle!").as_raw()
                ).unwrap().to_vec();

            let mut create_info = vk::InstanceCreateInfo {
                p_application_info: &app_info,
                enabled_layer_count: 0,
                ..Default::default()
            };        
        
            //enabling validation layer and debug extension
            //when running in debug mode
            let layer_names : Vec<*const c_char> = vec![c"VK_LAYER_KHRONOS_validation".as_ptr()];
            if DEBUG_MODE_ENABLED {
                //pushing the debug extension
                extension_names.push(debug_utils::NAME.as_ptr());
                //setting up the required validation layer
                create_info.enabled_layer_count = layer_names.len().try_into()
                    .expect("failed converting usize into u32");
                create_info.pp_enabled_layer_names = &raw const layer_names[0];

                //self explainatory
                Self::print_supported_extensions_and_layers(&api_entry);
            }

            //setting up the extensions
            //NOTE: below validation layer setup
            create_info.enabled_extension_count = extension_names.len().try_into()
                .expect("failed converting usize into u32");
            create_info.pp_enabled_extension_names = &raw const extension_names[0];

            //create vulkan instance
            let instance = api_entry
                .create_instance(&create_info, None)
                .expect("failed creating vulkan instance!");

            //create debug_ctx (= debug messenger for validation)
            let debug_ctx = Self::create_debug_messenger(&api_entry, &instance);

            (instance, debug_ctx)
        }
    } 

    fn print_supported_extensions_and_layers(api_entry: &Entry) {
        let supported_extensions;
        let supported_layers;
        unsafe {
            supported_extensions = api_entry.enumerate_instance_extension_properties(None)
                .expect("unable to find supported extensions!");
            
            supported_layers = api_entry.enumerate_instance_layer_properties()
                .expect("unable to find supported layers!");
        }
        println!("supported_extensions:");
        supported_extensions.iter().for_each(|extension| {
            println!("\t{:?}", &extension.extension_name_as_c_str().unwrap());
        });
        println!("supported_layers:");
        supported_layers.iter().for_each(|layer| {
            println!("\t{:?}", &layer.layer_name_as_c_str().unwrap());
        }); 
    }

    fn create_debug_messenger(api_entry: &Entry, instance: &Instance) -> Option<DebugCtx> {
        //return if debug mode is disabled
        if !DEBUG_MODE_ENABLED { return None }

        let debug_messenger_create_info = vk::DebugUtilsMessengerCreateInfoEXT {
            message_severity: 
                DebugUtilsMessageSeverityFlagsEXT::ERROR | 
                DebugUtilsMessageSeverityFlagsEXT::WARNING | 
                DebugUtilsMessageSeverityFlagsEXT::INFO,
            message_type: 
                DebugUtilsMessageTypeFlagsEXT::GENERAL |
                DebugUtilsMessageTypeFlagsEXT::VALIDATION |
                DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            pfn_user_callback: Some(vulkan_debug_callback),
            ..Default::default()
        };

        //create loader and call_back
        let (debug_utils_loader, debug_call_back) = unsafe {
            let debug_utils_loader = debug_utils::Instance::new(&api_entry, &instance);
            let debug_call_back = 
                debug_utils_loader
                    .create_debug_utils_messenger(&debug_messenger_create_info, None)
                    .expect("failed creating debug utils messenger!");
            (debug_utils_loader, debug_call_back)
        };

        Some(DebugCtx{ debug_utils_loader, debug_call_back })
    }

    pub fn draw(&self) {}
}
impl Drop for Renderer {
    //cleanup of vulkan objects (LIFO)
    fn drop(&mut self) {
        println!("cleaning up the renderer!");
        unsafe {
            //destroy debug_call_back (if it exists)
            if self.debug_ctx.is_some() {
                let debug_ctx= self.debug_ctx.as_ref();
                debug_ctx.unwrap().debug_utils_loader
                    .destroy_debug_utils_messenger(
                        debug_ctx.unwrap().debug_call_back, 
                        None
                    );
            } 
            //destroy vulkan instance
            self.instance.destroy_instance(None);
        }
    }
}