use eframe::{egui, epi};
use image::{ImageFormat, Rgb};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[cfg_attr(feature = "persistence", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "persistence", serde(default))] // if we add new fields, give them default values when deserializing old state
pub struct GestureDatasetApp {
	gestures: Vec<String>,
	current_gesture: String,

	width: u32,
	height: u32,

	#[cfg_attr(feature = "persistence", serde(skip))]
	drawing: Vec<Vec<egui::Pos2>>,

	#[cfg_attr(feature = "persistence", serde(skip))]
	sample_count: u32,
}

impl Default for GestureDatasetApp {
	fn default() -> Self {
		Self {
			gestures: Vec::new(),
			current_gesture: "".to_owned(),

			width: 32,
			height: 32,

			drawing: Default::default(),

			sample_count: 0,
		}
	}
}

impl epi::App for GestureDatasetApp {
	fn name(&self) -> &str {
		"Gesture Dataset Creator"
	}

	/// Called once before the first frame.
	fn setup(
		&mut self,
		_ctx: &egui::CtxRef,
		_frame: &epi::Frame,
		_storage: Option<&dyn epi::Storage>,
	) {
		// Load previous app state (if any).
		// Note that you must enable the `persistence` feature for this to work.
		#[cfg(feature = "persistence")]
		if let Some(storage) = _storage {
			*self = epi::get_value(storage, epi::APP_KEY).unwrap_or_default()
		}
	}

	/// Called by the frame work to save state before shutdown.
	/// Note that you must enable the `persistence` feature for this to work.
	#[cfg(feature = "persistence")]
	fn save(&mut self, storage: &mut dyn epi::Storage) {
		epi::set_value(storage, epi::APP_KEY, self);
	}

	/// Called each time the UI needs repainting, which may be many times per second.
	/// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
	fn update(&mut self, ctx: &egui::CtxRef, frame: &epi::Frame) {
		let Self {
			gestures: gestures,
			current_gesture: label,
			width,
			height,
			drawing: drawing,
			sample_count,
		} = self;

		egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
			// The top panel is often a good place for a menu bar:
			egui::menu::bar(ui, |ui| {
				ui.menu_button("File", |ui| {
					if ui.button("Quit").clicked() {
						frame.quit();
					}
				});
			});
		});

		egui::SidePanel::left("side_panel").show(ctx, |ui| {
			ui.vertical(|ui|{
				// This section handles the UI and creation of data directories for gesture classes.
				// After a user types in the name of a new gesture, see if it's already on the list.
				// If it isn't, create the data directory and add it.
				ui.label("Add New Gesture Class: ");
				ui.horizontal(|ui| {
					ui.text_edit_singleline(label);
					if ui.button("+").clicked() {
						label.make_ascii_lowercase();
						if !gestures.contains(&label) { // This is new!  Add it to our listing and make the directory.
							let _res = std::fs::create_dir(&label);
							gestures.push(label.clone());
						}
					}
				});

				ui.separator();

				// For each possible directory, add a radio button.  This determines where we save the result images.
				for g in gestures.iter() {
					if ui.radio(g.eq(label), g).clicked() {
						*label = g.clone();
					}
				}

				ui.separator();

				ui.add(egui::Slider::new(width, 0..=256).text("width"));
				ui.add(egui::Slider::new(height, 0..=256).text("height"));
			});

			ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
				ui.horizontal(|ui| {
					ui.spacing_mut().item_spacing.x = 0.0;
					egui::warn_if_debug_build(ui);
					ui.label("powered by ");
					ui.hyperlink_to("egui", "https://github.com/emilk/egui");
					ui.label(" and ");
					ui.hyperlink_to("eframe", "https://github.com/emilk/egui/tree/master/eframe");
				});
			});
		});

		egui::CentralPanel::default().show(ctx, |ui| {
			// As soon as a person is done with a stroke, clear it.
			//egui::stroke_ui(ui, &mut self.stroke, "Stroke");

			if ui.button("Clear Painting").clicked() {
				drawing.clear();
			}
			if ui.button("Save").clicked() {
				save_image(drawing, label, *sample_count, (*width, *height));
				*sample_count += 1;
				drawing.clear();
			}

			egui::Frame::dark_canvas(ui.style()).show(ui, |ui| {
				let (mut response, painter) = ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());
				let to_screen = egui::emath::RectTransform::from_to(
					egui::Rect::from_min_size(egui::Pos2::ZERO, response.rect.square_proportions()),
					response.rect,
				);
				let from_screen = to_screen.inverse();

				if drawing.is_empty() {
					drawing.push(vec![]);
				}

				let current_line = drawing.last_mut().unwrap();

				if let Some(pointer_pos) = response.interact_pointer_pos() {
					let canvas_pos = from_screen * pointer_pos;
					if current_line.last() != Some(&canvas_pos) {
						current_line.push(canvas_pos);
						response.mark_changed();
					}
				} else if !current_line.is_empty() {
					drawing.push(vec![]);
					response.mark_changed();
				}

				let mut shapes = vec![];
				for line in drawing.iter() {
					if line.len() >= 2 {
						let points: Vec<egui::Pos2> = line.iter().map(|p| to_screen * *p).collect();
						shapes.push(egui::Shape::line(points, egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255))));
					}
				}
				painter.extend(shapes);
			});
		});
	}
}

fn save_image(lines: &Vec<Vec<egui::Pos2>>, class_name: &String, sample_number: u32, raster_size: (u32, u32)) {
	// Lines will be all over the place, so we want to remap them to the appropriate size.
	// Find the bounds of the drawing and remap them to the edges of the image.
	let mut min_x = 1e32;
	let mut max_x = -1e32;
	let mut min_y = 1e32;
	let mut max_y = -1e32;
	for line in lines.iter() {
		if line.len() < 2 { continue; }
		for pt in line {
			min_x = pt.x.min(min_x);
			min_y = pt.y.min(min_y);
			max_x = pt.x.max(max_x);
			max_y = pt.y.max(max_y);
		}
	}
	max_x += 1.0;
	max_y += 1.0;

	// Draw the pixels.
	// Normalize to the 0/1 range and set pixels between start and stops.
	let mut img = image::RgbImage::new(raster_size.0, raster_size.1);
	for line in lines.iter() {
		if line.len() < 2 { continue; }
		for (pt_a, pt_b) in line.iter().zip(line.iter().skip(1)) {
			let dx = pt_b.x - pt_a.x;
			let dy = pt_b.y - pt_a.y;
			let pixel_steps = dx.abs().max(dy.abs()).ceil() as u32;

			for step in 0..pixel_steps {
				let mut x = pt_a.x + (dx*step as f32 / pixel_steps as f32);
				let mut y = pt_a.y + (dy*step as f32 / pixel_steps as f32);
				// Convert the X/Y into the smaller form factor and set the pixel.
				x = (x - min_x) / (max_x - min_x);
				y = (y - min_y) / (max_y - min_y);

				let mut pxl = img.get_pixel_mut((x*raster_size.0 as f32) as u32, (y*raster_size.1 as f32) as u32);
				*pxl = Rgb::from([255, 255, 255]);
			}
		}
	}

	// Save the example.
	let path = format!("{}{}{}.png", class_name, std::path::MAIN_SEPARATOR, sample_number);
	img.save_with_format(&path, ImageFormat::Png);
	println!("Saved {}", &path);
}

fn main() {
	let app = GestureDatasetApp::default();
	let native_options = eframe::NativeOptions::default();
	eframe::run_native(Box::new(app), native_options);
}