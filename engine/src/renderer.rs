use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ffi::{self, CStr, c_char};
use std::fs;
use std::io::Cursor;
use std::os::raw::c_void;
use ash::util::read_spv;
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use ash::{
    Entry,
    Instance,
    Device
};
use ash::ext::debug_utils;
use ash::vk::{
    self, AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp, ColorComponentFlags, ColorSpaceKHR, ComponentMapping, ComponentSwizzle, CompositeAlphaFlagsKHR, CullModeFlags, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT, DebugUtilsMessengerEXT, DeviceCreateInfo, DeviceQueueCreateInfo, DynamicState, Extent2D, Format, FrontFace, GraphicsPipelineCreateInfo, ImageAspectFlags, ImageLayout, ImageSubresourceRange, ImageUsageFlags, ImageView, ImageViewCreateInfo, ImageViewType, PhysicalDevice, PhysicalDeviceFeatures, Pipeline, PipelineBindPoint, PipelineCache, PipelineColorBlendAttachmentState, PipelineColorBlendStateCreateInfo, PipelineDynamicStateCreateInfo, PipelineInputAssemblyStateCreateInfo, PipelineLayout, PipelineLayoutCreateInfo, PipelineMultisampleStateCreateInfo, PipelineRasterizationStateCreateInfo, PipelineShaderStageCreateInfo, PipelineVertexInputStateCreateInfo, PipelineViewportStateCreateInfo, PolygonMode, PresentModeKHR, PrimitiveTopology, Queue, QueueFlags, RenderPass, RenderPassCreateInfo, SampleCountFlags, ShaderModule, ShaderModuleCreateInfo, ShaderStageFlags, SharingMode, SubpassDescription, SurfaceCapabilitiesKHR, SurfaceFormatKHR, SurfaceKHR, SwapchainCreateInfoKHR, SwapchainKHR
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

struct Queues {
    //queue - graphics commands can be sent to
    graphics_queue: Queue,
    //queue - present commands can be sent to
    present_queue: Queue    
}

//wrapper - swapchain details (for creation and use) 
struct SwapchainSupportDetails {
    surface_capabilities: SurfaceCapabilitiesKHR,
    surface_formats: Vec<SurfaceFormatKHR>,
    surface_present_modes: Vec<PresentModeKHR>
}

struct SwapchainData {
    //swapchain images format
    swapchain_image_format: Format,
    //swapchain resolution
    swapchain_extent: Extent2D,
    //queue of images that are waiting to be presented
    //to screen (infrastructure for handling that)
    swapchain: SwapchainKHR,
    //swapchain images
    swapchain_images: Vec<vk::Image>,
    //describe how to access images
    swapchain_image_views: Vec<vk::ImageView>
}

struct PipelineData {
    pipeline_layout: PipelineLayout,
    pipeline: Pipeline
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
    //queues - commands can be sent to
    queues: Queues,
    //provides swapchain loading
    swapchain_loader: ash::khr::swapchain::Device,
    //swapchain wrapper
    swapchain_data: SwapchainData,
    //defines attachments referenced by
    //pipeline stages and their usage
    render_pass: RenderPass,
    //pipeline wrapper
    graphics_pipeline_data: PipelineData
}
impl Renderer {
    const DEVICE_EXTENSIONS: [&CStr; 1] = [vk::KHR_SWAPCHAIN_NAME];

    pub fn new(event_loop: &ActiveEventLoop, window: &Window) -> Self {
        let api_entry = Entry::linked();
        let (instance, debug_ctx)  = Self::create_instance(&api_entry, &event_loop);
        let surface_loader = ash::khr::surface::Instance::new(&api_entry, &instance);
        let surface = Self::create_surface(&api_entry, &instance, &event_loop, &window);
        let physical_device = Self::select_physical_device(&instance, &surface_loader, &surface);
        let (logical_device, queues) = Self::create_logical_device(&instance, &physical_device, &surface_loader, &surface);
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &logical_device);
        let swapchain_data = Self::create_swapchain(&window, &instance, &physical_device, &logical_device, &surface_loader, &surface, &swapchain_loader);
        let render_pass = Self::create_render_pass(&logical_device, &swapchain_data);
        let graphics_pipeline_data = Self::create_graphics_pipeline(&logical_device, &swapchain_data, &render_pass);

        Self {
            instance,
            debug_ctx,
            surface_loader,
            surface,
            physical_device,
            logical_device,
            queues,
            swapchain_loader,
            swapchain_data,
            render_pass,
            graphics_pipeline_data
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

    fn check_physical_device_extension_support(instance: &Instance, physical_device: &PhysicalDevice) -> bool {
        let available_device_extensions = unsafe { instance
            .enumerate_device_extension_properties(*physical_device)
            .expect("failed enumerating device extension properties!")
        };

        let mut required_extensions : BTreeSet<_> = 
            Self::DEVICE_EXTENSIONS.iter().map(|ext| *ext).collect();

        for extension in available_device_extensions.iter() {
            required_extensions.remove(extension.extension_name_as_c_str().unwrap());
        }

        required_extensions.is_empty()
    }

    fn is_physical_device_suitable(instance: &Instance, physical_device: &PhysicalDevice, surface_loader: &ash::khr::surface::Instance, surface: &SurfaceKHR) -> bool {
        unsafe {
            let physical_device_properties= 
                instance.get_physical_device_properties(*physical_device);

            let _physical_device_features = 
                instance.get_physical_device_features(*physical_device);

            let queue_families = 
                Self::find_queue_families(&instance, &physical_device, &surface_loader, &surface);

            let extension_support = 
                Self::check_physical_device_extension_support(&instance, &physical_device);

            let mut swapchain_adequate = false;
            if extension_support {
                let swapchain_details = 
                    Self::query_swapchain_support_details(&physical_device, &surface_loader, &surface);
                //swapchain is sufficient when there is at least one format and one present mode
                swapchain_adequate = !swapchain_details.surface_formats.is_empty() &&
                                            !swapchain_details.surface_present_modes.is_empty();
            }

            let suitable = 
                queue_families.is_complete() &&
                extension_support &&
                swapchain_adequate;

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

    fn create_logical_device(instance: &Instance, physical_device: &PhysicalDevice, surface_loader: &ash::khr::surface::Instance, surface: &SurfaceKHR) -> (Device, Queues)  {
        //creating queues:
        let queue_families = 
            Self::find_queue_families(&instance, &physical_device, &surface_loader, &surface); 
        
        //set of unique items
        let unique_queue_families: BTreeSet<u32> = [
            queue_families.graphics_family.unwrap(),
            queue_families.present_family.unwrap()
            ].into_iter().collect();
        
        let mut queues_create_info_vec = 
            Vec::with_capacity(unique_queue_families.len());

        let queue_priority: f32 = 1.0;

        unique_queue_families.iter().for_each(|queue_family| {
            queues_create_info_vec.push(
                DeviceQueueCreateInfo {
                    queue_family_index: *queue_family,
                    queue_count: 1,
                    p_queue_priorities: &raw const queue_priority,
                    ..Default::default()    
                }
            );
        });

        let logical_device_features = 
            PhysicalDeviceFeatures {
                ..Default::default()
            };

        let required_logical_device_extensions : Vec<_> = 
            Self::DEVICE_EXTENSIONS.iter().map(|extension| {extension.as_ptr()}).collect();

        let logical_device_create_info = DeviceCreateInfo {
            p_queue_create_infos: &raw const queues_create_info_vec[0],
            queue_create_info_count: helper::usize_into_u32(queues_create_info_vec.len()),
            p_enabled_features: &raw const logical_device_features,
            pp_enabled_extension_names: &raw const required_logical_device_extensions[0],
            enabled_extension_count: helper::usize_into_u32(required_logical_device_extensions.len()),
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

        let present_queue= unsafe {
            logical_device.get_device_queue(
                queue_families.present_family.unwrap(), 
                0
            )
        };

        let queues = Queues {
            graphics_queue,
            present_queue
        };
        
        (logical_device, queues)
    }

    fn query_swapchain_support_details(physical_device: &PhysicalDevice, surface_loader: &ash::khr::surface::Instance, surface: &SurfaceKHR) -> SwapchainSupportDetails {
        unsafe {
            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(*physical_device, *surface)
                .expect("failed getting physical-device surface capabilities");

            let surface_formats = surface_loader
                .get_physical_device_surface_formats(*physical_device, *surface)
                .expect("failed getting physical-device surface formats");

            let surface_present_modes = surface_loader
                .get_physical_device_surface_present_modes(*physical_device, *surface)
                .expect("failed getting physical-device surface present modes");


            SwapchainSupportDetails {
                surface_capabilities,
                surface_formats,
                surface_present_modes
            }
        }
    }

    fn chose_swapchain_surface_format(available_surface_formats: &Vec<SurfaceFormatKHR>) -> SurfaceFormatKHR {
        for surface_format in available_surface_formats {
            if surface_format.format == Format::R8G8B8A8_SRGB && surface_format.color_space == ColorSpaceKHR::SRGB_NONLINEAR {
                return *surface_format
            }
        }
        *available_surface_formats.first().unwrap()
    }

    fn chose_swapchain_present_mode(available_surface_present_modes: &Vec<PresentModeKHR>) -> PresentModeKHR {
        for present_mode in available_surface_present_modes.iter() {
            /*
                Instead of blocking the application when the queue is full, the images that are already queued
                are simply replaced with the newer ones. This mode can be used to render frames as fast as 
                possible while still avoiding tearing, resulting in fewer latency issues than standard 
                vertical sync. This is commonly known as "triple buffering" 
            */
            if *present_mode == PresentModeKHR::MAILBOX {
                return *present_mode
            }
        }

        /*
            swap chain is a queue where the display takes an image from the front of the queue when the display 
            is refreshed and the program inserts rendered images at the back of the queue. If the queue is full 
            then the program has to wait. This is most similar to vertical sync as found in modern games. 
            The moment that the display is refreshed is known as "vertical blank".
        */
        PresentModeKHR::FIFO //this mode is guranteed to be available
    }

    fn chose_swapchain_extent(surface_capabilities: &SurfaceCapabilitiesKHR, window: &Window) -> Extent2D {
        //if current_extent == u32::MAX then the resolution of the window
        //may be differing from the current_extent (e.g. retina-display) 
        if surface_capabilities.current_extent.width != u32::MAX {
            return surface_capabilities.current_extent
        } else {
            let logical_inner_size: LogicalSize<u32> = window.inner_size().to_logical(window.scale_factor());
            Extent2D { 
                width: u32::clamp(
                    logical_inner_size.width,
                    surface_capabilities.min_image_extent.width,
                    surface_capabilities.min_image_extent.height
                ),
                height: u32::clamp(
                    logical_inner_size.height,
                    surface_capabilities.min_image_extent.height,
                    surface_capabilities.max_image_extent.height
                )
            }
        }
    }

    fn create_swapchain(window: &Window, instance: &Instance, physical_device: &PhysicalDevice, logical_device: &Device, surface_loader: &ash::khr::surface::Instance, surface: &SurfaceKHR, swapchain_loader: &ash::khr::swapchain::Device) -> SwapchainData {
        let surface_details = 
            Self::query_swapchain_support_details(&physical_device, &surface_loader, &surface);
        //https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Swap_chain#page_Surface-format
        let surface_format = 
            Self::chose_swapchain_surface_format(&surface_details.surface_formats);
        //https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Swap_chain#page_Presentation-mode
        let surface_present_mode = 
            Self::chose_swapchain_present_mode(&surface_details.surface_present_modes);
        //resolution of the swapchain-images
        //https://vulkan-tutorial.com/Drawing_a_triangle/Presentation/Swap_chain#page_Swap-extent
        let swapchain_extent = 
            Self::chose_swapchain_extent(&surface_details.surface_capabilities, &window);

        let mut swapchain_min_image_count =
            surface_details.surface_capabilities.min_image_count + 1;
        
        //max_image_count = 0: there is no maximum
        if surface_details.surface_capabilities.max_image_count > 0 &&
           surface_details.surface_capabilities.max_image_count < swapchain_min_image_count {
            swapchain_min_image_count = surface_details.surface_capabilities.max_image_count;
        }

        let mut swapchain_create_info = SwapchainCreateInfoKHR { 
            surface: *surface,
            min_image_count: swapchain_min_image_count,
            image_format: surface_format.format,
            image_color_space: surface_format.color_space,
            image_extent: swapchain_extent,
            image_array_layers: 1, 
            image_usage: ImageUsageFlags::COLOR_ATTACHMENT,
            pre_transform: surface_details.surface_capabilities.current_transform,
            composite_alpha: CompositeAlphaFlagsKHR::OPAQUE,
            present_mode: surface_present_mode,
            clipped: vk::TRUE,
            ..Default::default()
        };

        let queue_family_indices = 
            Self::find_queue_families(instance, &physical_device, &surface_loader, &surface);
        
        let queue_family_indices_vec = vec![
            queue_family_indices.graphics_family.unwrap(),
            queue_family_indices.present_family.unwrap()
        ];

        if queue_family_indices.graphics_family.unwrap() != queue_family_indices.present_family.unwrap() {
            swapchain_create_info.image_sharing_mode = SharingMode::CONCURRENT;
            swapchain_create_info.p_queue_family_indices = &raw const queue_family_indices_vec[0];
            swapchain_create_info.queue_family_index_count = 2;
        } else {
            swapchain_create_info.image_sharing_mode = SharingMode::EXCLUSIVE;
        }

        let swapchain = unsafe {
            swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .expect("failed creating swapchain!")
        };


        let swapchain_images = unsafe {
            swapchain_loader
                .get_swapchain_images(swapchain)
                .expect("failed getting swapchain images")
        };

        dbg!(&swapchain_images);

        let mut swapchain_image_views: Vec<ImageView> = Vec::with_capacity(swapchain_images.len());
        swapchain_images.iter().for_each(|swapchain_image| {
            let swapchain_image_view_create_info =
                ImageViewCreateInfo {
                    image: *swapchain_image,
                    view_type: ImageViewType::TYPE_2D,
                    format: surface_format.format,
                    components: ComponentMapping {
                        r: ComponentSwizzle::IDENTITY,
                        g: ComponentSwizzle::IDENTITY,
                        b: ComponentSwizzle::IDENTITY,
                        a: ComponentSwizzle::IDENTITY
                    },
                    subresource_range: ImageSubresourceRange {
                        aspect_mask: ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0, 
                        layer_count: 1
                    },
                    ..Default::default()
                };

            swapchain_image_views.push(
                unsafe {
                    logical_device.create_image_view(
                        &swapchain_image_view_create_info,
                        None
                    ).expect("failed creating image view")
                }
            );                       
        });

        SwapchainData {
            swapchain_image_format: surface_format.format,
            swapchain_extent,
            swapchain,
            swapchain_images,
            swapchain_image_views
        }
    }

    fn create_render_pass(logical_device: &Device, swapchain_data: &SwapchainData) -> RenderPass {
        let color_attachment_description = 
            AttachmentDescription {
                format: swapchain_data.swapchain_image_format,
                samples: SampleCountFlags::TYPE_1,
                //load-op for color/depth buffers
                load_op: AttachmentLoadOp::CLEAR,
                //store-op for color/depth buffers
                store_op: AttachmentStoreOp::STORE,
                //load-op for stencil buffers
                stencil_load_op: AttachmentLoadOp::DONT_CARE,
                //store-op for stencil buffers
                stencil_store_op: AttachmentStoreOp::DONT_CARE,
                //images need to be transitioned to specific layouts that 
                //are suitable for operation that they're be involved in
                initial_layout: ImageLayout::UNDEFINED,
                //-''-
                final_layout: ImageLayout::PRESENT_SRC_KHR,
                
                ..Default::default()
            };
        
        let color_attachment_reference = 
            AttachmentReference {
                attachment: 0,
                layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL
            };

        let subpass_description = 
            SubpassDescription {
                pipeline_bind_point: PipelineBindPoint::GRAPHICS,
                //index of the attachment in this "array" is
                //referenced from the fragment shader with:
                //``layout (location = 0) out vec4 out_color``
                p_color_attachments: &raw const color_attachment_reference,
                color_attachment_count: 1,
                ..Default::default()
            };

        let render_pass_create_info = 
            RenderPassCreateInfo {
                attachment_count: 1,
                p_attachments: &raw const color_attachment_description,
                subpass_count: 1,
                p_subpasses: &raw const subpass_description,
                ..Default::default()
            };
        
        unsafe {
            logical_device
                .create_render_pass(&render_pass_create_info, None)
                .expect("failed creating render pass")
        }
    }

    fn create_shader_module(logical_device: &Device, byte_code: &Vec<u8>) -> ShaderModule {
        let spirv = read_spv(&mut Cursor::new(byte_code))
            .expect("failed to convert u8-vec to u32-vec");

        let shader_module_create_info = ShaderModuleCreateInfo {
            code_size: spirv.len() * std::mem::size_of::<u32>(),
            p_code: &raw const spirv[0],
            ..Default::default()
        };


        unsafe {
            logical_device
                .create_shader_module(&shader_module_create_info, None)
                .expect("failed creating shader module!")
        }
    }

    fn create_graphics_pipeline(logical_device: &Device, swapchain_data: &SwapchainData, render_pass: &RenderPass) -> PipelineData {
        let vert_byte_src = 
            fs::read("./shaders/default_vert.spv")
                .expect("failed reading vertex shader");
        let frag_byte_src = 
            fs::read("./shaders/default_frag.spv")
                .expect("failed reading fragment shader");

        let vert_shader_module = Self::create_shader_module(&logical_device, &vert_byte_src);
        let frag_shader_module = Self::create_shader_module(&logical_device, &frag_byte_src);

        let vert_shader_stage_create_info = 
            PipelineShaderStageCreateInfo {
                stage: ShaderStageFlags::VERTEX,
                module: vert_shader_module,
                p_name: c"main".as_ptr(),
                ..Default::default()
            };
        
        let frag_shader_stage_create_info = 
            PipelineShaderStageCreateInfo {
                stage: ShaderStageFlags::FRAGMENT,
                module: frag_shader_module,
                p_name: c"main".as_ptr(),
                ..Default::default()
            };

        let shader_stage_create_infos = 
            [vert_shader_stage_create_info, frag_shader_stage_create_info];

        let vertex_input_create_info = 
            PipelineVertexInputStateCreateInfo {
            ..Default::default()
            };

        let input_assembly_create_info = 
            PipelineInputAssemblyStateCreateInfo {
                topology: PrimitiveTopology::TRIANGLE_LIST,
                primitive_restart_enable: vk::FALSE,
                ..Default::default()
            };

        let dynamic_states = [DynamicState::VIEWPORT, DynamicState::SCISSOR];
        let dynamic_state_create_info = 
            PipelineDynamicStateCreateInfo {
                dynamic_state_count: helper::usize_into_u32(dynamic_states.len()),
                p_dynamic_states: &raw const dynamic_states[0],
                ..Default::default()
            };
        
        let viewport_create_info = 
            PipelineViewportStateCreateInfo {
                viewport_count: 1,
                scissor_count: 1,
                ..Default::default()
            };

        let rasterization_create_info = 
            PipelineRasterizationStateCreateInfo {
                //TRUE: fragments beyond the near and far planes are clamped
                //      to them as opposed to discarding them
                depth_clamp_enable: vk::FALSE,
                //TRUE: geometry never passes through rasterizer stage
                //      -> disables any output to framebuffer
                rasterizer_discard_enable: vk::FALSE,
                polygon_mode: PolygonMode::FILL,
                line_width: 1.0,
                cull_mode: CullModeFlags::BACK,
                front_face: FrontFace::CLOCKWISE,
                depth_bias_enable: vk::FALSE,
                ..Default::default()
            };

        let multisample_create_info = 
            PipelineMultisampleStateCreateInfo {
                sample_shading_enable: vk::FALSE,
                rasterization_samples: SampleCountFlags::TYPE_1,
                ..Default::default()
            };

        //per-attatched-framebuffer configuration
        let color_blend_attachment_state = 
            PipelineColorBlendAttachmentState {
                color_write_mask: ColorComponentFlags::R |
                                ColorComponentFlags::G |
                                ColorComponentFlags::B |
                                ColorComponentFlags::A,
                blend_enable: vk::FALSE,
                ..Default::default()
            };
        
        let color_blend_create_info = 
            PipelineColorBlendStateCreateInfo {
                logic_op_enable: vk::FALSE,
                attachment_count: 1,
                p_attachments: &raw const color_blend_attachment_state,
                ..Default::default()
            };

        //specification of uniform values in shaders
        let pipeline_layout_create_info = PipelineLayoutCreateInfo::default();

        let pipeline_layout = unsafe {
            logical_device
                .create_pipeline_layout(
                    &pipeline_layout_create_info,
                    None
                )
                .expect("failed creating pipeline layout")
        };

        let graphics_pipeline_create_info = 
            GraphicsPipelineCreateInfo {
                //vertex and fragment shader-stages
                stage_count: 2,
                p_stages: &raw const shader_stage_create_infos[0],
                //fixed function stages
                p_vertex_input_state: &raw const vertex_input_create_info,
                p_input_assembly_state: &raw const input_assembly_create_info,
                p_viewport_state: &raw const viewport_create_info,
                p_rasterization_state: &raw const rasterization_create_info,
                p_multisample_state: &raw const multisample_create_info,
                p_color_blend_state: &raw const color_blend_create_info,
                p_dynamic_state: &raw const dynamic_state_create_info,
                //pipeline layout
                layout: pipeline_layout,
                render_pass: *render_pass,
                //index of the sub-(render)-pass where 
                //this graphics pipeline will be used
                subpass: 0,
                ..Default::default()
            };

        let pipelines = unsafe {
            logical_device.create_graphics_pipelines(
                PipelineCache::null(),
                &[graphics_pipeline_create_info],
                None
            )
            .expect("failed creating graphics pipelines")
        };
        let pipeline = pipelines[0];
        
        unsafe {
            logical_device.destroy_shader_module(frag_shader_module, None);
            logical_device.destroy_shader_module(vert_shader_module, None);
        }


        PipelineData {
            pipeline_layout,
            pipeline
        }
    }

    pub fn _draw(&self) {}
}
impl Drop for Renderer {
    //cleanup of vulkan objects (LIFO)
    fn drop(&mut self) {
        println!("cleaning up the renderer!");
        unsafe {
            //destroy pipeline
            self.logical_device.destroy_pipeline(self.graphics_pipeline_data.pipeline, None);
            //destroy pipeline layout
            self.logical_device
                .destroy_pipeline_layout(self.graphics_pipeline_data.pipeline_layout, None);
            //destroy render pass
            self.logical_device
                .destroy_render_pass(self.render_pass, None);
            //destroy swapchain image views
            self.swapchain_data.swapchain_image_views.iter().for_each(|swapchain_image_view| {
                self.logical_device.destroy_image_view(*swapchain_image_view, None);
            });
            //destroy swapchain
            self.swapchain_loader.destroy_swapchain(self.swapchain_data.swapchain, None);
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