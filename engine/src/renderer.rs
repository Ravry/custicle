use std::borrow::Cow;
use std::ffi::{self, c_char};
use std::os::raw::c_void;
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use ash::{
    Entry,
    Instance,
    Device
};
use ash::ext::debug_utils;
use ash::vk::{
    self, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT, DebugUtilsMessengerEXT, DeviceCreateInfo, DeviceQueueCreateInfo, PhysicalDevice, PhysicalDeviceFeatures, Queue, QueueFlags, SurfaceKHR
};
use winit::window::Window;

use crate::helper;

const DEBUG_MODE_ENABLED: bool = cfg!(debug_assertions); 

//debug callback method
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    unsafe {
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
}

//wrapper around debug information
//(used to destroy the messenger)
struct DebugCtx {debug_utils_loader: debug_utils::Instance, debug_call_back: DebugUtilsMessengerEXT }

//wrapper around queue-family-indices
#[derive(Debug)]
struct QueueFamilyIndices {
    graphics_family: Option<u32>,
    present_family: Option<u32>
}
impl QueueFamilyIndices {
    fn new() -> Self {
        Self {
            graphics_family: None,
            present_family: None
        }
    }

    fn is_complete(&self) -> bool {
        self.graphics_family.is_some() &&
        self.present_family.is_some()
    }
}


pub struct Renderer {
    //connection between application and vulkan lib
    instance: Instance,
    debug_ctx: Option<DebugCtx>,
    //provides surface info and destroys it
    surface_loader: ash::khr::surface::Instance,
    //(WSI): connect vulkan and window system
    surface: SurfaceKHR,
    //selected graphics-card
    physical_device: PhysicalDevice,
    //usage of graphics-card
    logical_device: Device,
    //queue - graphics commands can be sent to
    graphics_queue: Queue
}
impl Renderer {
    pub fn new(event_loop: &ActiveEventLoop, window: &Window) -> Self {
        let api_entry = Entry::linked();
        let (instance, debug_ctx)  = Self::create_instance(&api_entry, &event_loop);
        let surface_loader = ash::khr::surface::Instance::new(&ash::Entry::linked(), &instance);
        let surface = Self::create_surface(&api_entry, &instance, &event_loop, &window);
        let physical_device = Self::select_physical_device(&instance, &surface_loader, &surface);
        let (logical_device, graphics_queue) = Self::create_logical_device(&instance, &physical_device, &surface_loader, &surface);

        Self {
            instance,
            debug_ctx,
            surface_loader,
            surface,
            physical_device,
            logical_device,
            graphics_queue
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
            let debug_create_info;
            if DEBUG_MODE_ENABLED {
                debug_create_info = Self::get_debug_create_info();
                //debug messenger for instance creation/deletion
                create_info.p_next = &raw const debug_create_info as *const c_void;

                //pushing the debug extension
                extension_names.push(debug_utils::NAME.as_ptr());
                //setting up the required validation layer
                create_info.enabled_layer_count = helper::usize_into_u32(layer_names.len());
                create_info.pp_enabled_layer_names = &raw const layer_names[0];

                //self explainatory
                Self::print_supported_extensions_and_layers(&api_entry);
            }

            //setting up the extensions
            //NOTE: below validation layer setup
            create_info.enabled_extension_count = helper::usize_into_u32(extension_names.len());
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

    fn get_debug_create_info() -> vk::DebugUtilsMessengerCreateInfoEXT<'static> {
        vk::DebugUtilsMessengerCreateInfoEXT {
            message_severity: 
                DebugUtilsMessageSeverityFlagsEXT::ERROR | 
                DebugUtilsMessageSeverityFlagsEXT::WARNING,
            message_type: 
                DebugUtilsMessageTypeFlagsEXT::GENERAL |
                DebugUtilsMessageTypeFlagsEXT::VALIDATION |
                DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            pfn_user_callback: Some(vulkan_debug_callback),
            ..Default::default()
        }
    }

    fn create_debug_messenger(api_entry: &Entry, instance: &Instance) -> Option<DebugCtx> {
        //return if debug mode is disabled
        if !DEBUG_MODE_ENABLED { return None }

        let debug_messenger_create_info = Self::get_debug_create_info();

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

    fn create_surface(api_entry: &Entry, instance: &Instance, event_loop: &ActiveEventLoop, window: &Window) -> SurfaceKHR {
        unsafe {
            ash_window::create_surface(
                api_entry,
                instance,
                event_loop.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None
            ).expect("failed creating window surface")
        }
    }

    fn find_queue_families(instance: &Instance, physical_device: &PhysicalDevice, surface_loader: &ash::khr::surface::Instance, surface: &SurfaceKHR) -> QueueFamilyIndices {
        let mut queue_family_indices = QueueFamilyIndices::new();

        unsafe {
            let queue_families = 
                instance.get_physical_device_queue_family_properties(*physical_device);
            for (index, queue_family) in queue_families.iter().enumerate() {
                let index = helper::usize_into_u32(index);

                if queue_family.queue_flags.contains(QueueFlags::GRAPHICS)  {
                    queue_family_indices.graphics_family = 
                        Some(index);
                }

                let present_support = 
                    surface_loader.get_physical_device_surface_support(
                        *physical_device,
                        index,
                        *surface
                    ).expect("failed fetching surface present support!");
                
                if present_support {
                    queue_family_indices.present_family = 
                        Some(index);
                }

                if queue_family_indices.is_complete() {
                    break
                }
            }
        }

        queue_family_indices
    } 

    fn is_physical_device_suitable(instance: &Instance, physical_device: &PhysicalDevice, surface_loader: &ash::khr::surface::Instance, surface: &SurfaceKHR) -> bool {
        unsafe {
            let physical_device_properties= 
                instance.get_physical_device_properties(*physical_device);

            let _physical_device_features = 
                instance.get_physical_device_features(*physical_device);

            let queue_families = 
                Self::find_queue_families(&instance, &physical_device, &surface_loader, &surface);

            let suitable = 
                queue_families.is_complete();

            if suitable {
                println!(
                    "suitable phyiscal-device:\n\t{:?}", 
                    physical_device_properties.device_name_as_c_str().unwrap()
                );
                dbg!(queue_families);
            }

            suitable
        }
    }

    fn select_physical_device(instance: &Instance, surface_loader: &ash::khr::surface::Instance, surface: &SurfaceKHR) -> PhysicalDevice {
        unsafe {
            let physical_devices = instance.enumerate_physical_devices()
                .expect("couldn't find any physical device!");
            assert!(physical_devices.len() > 0, "couldn't find any physical device!");
            
            for physical_device in physical_devices.iter() {
                if Self::is_physical_device_suitable(&instance, &physical_device, &surface_loader, &surface) {
                    return *physical_device                    
                }
            }
            
            panic!("no suitable physical device found!");
        }
    }

    fn create_logical_device(instance: &Instance, physical_device: &PhysicalDevice, surface_loader: &ash::khr::surface::Instance, surface: &SurfaceKHR) -> (Device, Queue)  {
        let queue_families = 
            Self::find_queue_families(&instance, &physical_device, &surface_loader, &surface); 
        
        let queue_priority: f32 = 1.0;
        
        //number of queues for a queue family
        let graphics_queue_create_info = 
            DeviceQueueCreateInfo {
                queue_family_index: queue_families.graphics_family.unwrap(),
                queue_count: 1,
                p_queue_priorities: &raw const queue_priority,
                ..Default::default()
            };
        
        let logical_device_features = 
            PhysicalDeviceFeatures {
                ..Default::default()
            };
        
        let logical_device_create_info = DeviceCreateInfo {
            p_queue_create_infos: &raw const graphics_queue_create_info,
            queue_create_info_count: 1,
            p_enabled_features: &raw const logical_device_features,
            ..Default::default()
        };

        let logical_device = unsafe {
            instance
                .create_device(*physical_device, &logical_device_create_info, None)
                .expect("failed creating logical device")
        };

        let graphics_queue = unsafe {
            logical_device.get_device_queue(
                queue_families.graphics_family.unwrap(), 
                0
            )
        };
        
        (logical_device, graphics_queue)
    }



    pub fn _draw(&self) {}
}
impl Drop for Renderer {
    //cleanup of vulkan objects (LIFO)
    fn drop(&mut self) {
        println!("cleaning up the renderer!");
        unsafe {
            //destroy logical device
            self.logical_device.destroy_device(None);
            //destroy surface
            self.surface_loader.destroy_surface(self.surface, None); 
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