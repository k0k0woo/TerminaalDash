use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color},
    widgets::{Widget},
};
use rand::{RngExt};

pub struct MatrixEdgeOverlay { pub tick: u64 }
impl Widget for MatrixEdgeOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut local_rng = rand::rng();
        for x in area.left()..area.right() {
            let col_seed = (x as u64).wrapping_mul(1103515245);
            if col_seed % 100 > 75 { continue; }
            let head_y = ((self.tick / ((col_seed % 3) + 1)) as i64 + (col_seed % 100) as i64) % (area.height as i64 + 20) - 10;
            for y in area.top()..area.bottom() {
                let dist_x = std::cmp::min(x - area.left(), area.right() - 1 - x);
                let dist_y = std::cmp::min(y - area.top(), area.bottom() - 1 - y);
                if dist_x > 3 && dist_y > 1 { continue; }
                let dist_to_head = head_y - y as i64;
                if dist_to_head >= 0 && dist_to_head < 15 {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        let chars = ['0','1','2','3','4','5','6','7','8','9','A','B','C','Z','X','Y','W','*','+','=','-',':','.','"','$','%','&'];
                        let c = chars[local_rng.random_range(0..chars.len())];
                        cell.set_symbol(&c.to_string());
                        if dist_to_head == 0 { cell.set_fg(Color::White); } else if dist_to_head < 3 { cell.set_fg(Color::LightGreen); } else if dist_to_head > 11 { cell.set_fg(Color::DarkGray); } else { cell.set_fg(Color::Green); }
                    }
                }
            }
        }
    }
}

pub struct PulseGraphOverlay { pub tick: u64 }

impl Widget for PulseGraphOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for x in area.left()..area.right() {
            // Create a "random" but smooth height using multiple sine waves
            let wave = ( (self.tick as f32 * 0.2) + (x as f32 * 0.4) ).sin() 
                     + ( (self.tick as f32 * 0.1) - (x as f32 * 0.2) ).cos();
            let height = ( (wave + 2.0) * 2.0 ) as u16; 

            for h in 0..height {
                // Draw on both top and bottom edges
                for y in [area.top() + h, area.bottom().saturating_sub(h + 1)] {
                    if y >= area.top() && y < area.bottom() {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            let color_val = (x + self.tick as u16) % 7;
                            let color = match color_val {
                                0 => Color::Magenta, 1 => Color::Blue, 2 => Color::Cyan,
                                3 => Color::Green, 4 => Color::Yellow, 5 => Color::Red,
                                _ => Color::Rgb(255, 100, 200),
                            };
                            cell.set_symbol("┃").set_fg(color);
                        }
                    }
                }
            }
        }
    }
}

pub struct PlasmaWaveOverlay { pub tick: u64 }

impl Widget for PlasmaWaveOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = self.tick as f32 * 0.1;
        for x in area.left()..area.right() {
            for y in area.top()..area.bottom() {
                // Only render on the very edge
                if x > area.left() && x < area.right() - 1 && y > area.top() && y < area.bottom() - 1 {
                    continue;
                }

                let v = ( (x as f32 * 0.3 + t).sin() + 
                          (y as f32 * 0.3 + t).sin() + 
                          ((x + y) as f32 * 0.3 + t).sin() ).abs();
                
                if let Some(cell) = buf.cell_mut((x, y)) {
                    let symbols = ["·", "≈", "≋", "≡", "█"];
                    let idx = (v * 1.5) as usize % symbols.len();
                    cell.set_symbol(symbols[idx]);
                    cell.set_fg(Color::Indexed(20 + (v * 10.0) as u8));
                }
            }
        }
    }
}

pub struct ThunderstormOverlay { pub tick: u64 }

impl Widget for ThunderstormOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut rng = rand::rng();
        
        // Lightning cycle: Flash intensively every ~150 ticks
        let flash_cycle = self.tick % 150;
        let is_flash = flash_cycle > 140 && flash_cycle % 2 == 0;
        
        for x in area.left()..area.right() {
            let col_seed = (x as u64).wrapping_mul(87654321);
            
            for y in area.top()..area.bottom() {
                // Keep the effect out of the center text areas
                let is_edge = x - area.left() < 4 || area.right() - x <= 4 || y - area.top() < 2 || area.bottom() - y <= 2;
                if !is_edge { continue; }

                if is_flash {
                    // Lightning: Randomly white out the edges
                    if rng.random_bool(0.6) {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_bg(Color::Rgb(220, 230, 255)).set_fg(Color::Black);
                        }
                    }
                } else {
                    // Rain: Fast moving vertical lines
                    let speed = 3;
                    let drop_y = ((self.tick * speed) + col_seed % 100) % (area.height as u64 + 10);
                    
                    if y as u64 == drop_y || y as u64 == drop_y.wrapping_sub(1) {
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            let symbol = if rng.random_bool(0.5) { "│" } else { "┃" };
                            let color = if col_seed % 2 == 0 { Color::DarkGray } else { Color::Rgb(100, 150, 200) };
                            cell.set_symbol(symbol).set_fg(color);
                        }
                    }
                }
            }
        }
    }
}

pub struct FluidFireOverlay { pub tick: u64 }

impl Widget for FluidFireOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = self.tick as f32 * 0.2;
        
        for x in area.left()..area.right() {
            for y in area.top()..area.bottom() {
                let dist_to_bottom = (area.bottom() - y) as f32;
                // Add 1.0 to top distance so it mathematically matches the bottom distance offset
                let dist_to_top = (y - area.top()) as f32 + 1.0; 
                
                // Matches your safe_area padding perfectly (4 columns wide, 2 rows tall)
                let is_side = x - area.left() < 4 || area.right() - x <= 4;
                let is_bottom = dist_to_bottom <= 2.0;
                let is_top = dist_to_top <= 2.0; 
                
                // If it's not in the 4-char side margin, 2-char bottom, or 2-char top margin, skip it
                if !is_side && !is_bottom && !is_top { continue; }
                
                // Scale coordinates for the wave frequency
                let nx = x as f32 * 0.25;
                let ny = y as f32 * 0.5;
                
                // Intersecting waves for pseudo-noise
                let v1 = (nx + t).sin();
                let v2 = (ny - t).cos();
                let v3 = ((nx + ny - t) * 0.7).sin();
                let noise = (v1 + v2 + v3) / 3.0; // Ranges roughly -1.0 to +1.0
                
                // On the top edge, calculate falloff based on distance from the top.
                // On the sides and bottom, base it on distance from the bottom so flames rise.
                let active_dist = if is_top { dist_to_top } else { dist_to_bottom };
                let falloff = if is_side && !is_bottom && !is_top { 12.0 } else { 3.0 };
                
                let intensity = noise + (1.0 - (active_dist / falloff).clamp(0.0, 1.0));
                
                if intensity > 0.1 {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        let chars = ["░", "▒", "▓", "█"];
                        let idx = ((intensity * 2.0).clamp(0.0, 3.0)) as usize;
                        
                        let col = if intensity > 1.2 { 
                            Color::Yellow 
                        } else if intensity > 0.7 { 
                            Color::LightRed 
                        } else if intensity > 0.3 {
                            Color::Red
                        } else {
                            Color::DarkGray
                        };
                        
                        cell.set_symbol(chars[idx]).set_fg(col);
                    }
                }
            }
        }
    }
}