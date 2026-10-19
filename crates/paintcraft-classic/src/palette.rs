//! Paint-style names for the fixed palette; extra swatches have descriptive names.
const NAMES: [&str; 28] = [
    "Black",
    "Gray",
    "Dark red",
    "Red",
    "Orange",
    "Yellow",
    "Green",
    "Turquoise",
    "Indigo",
    "Purple",
    "White",
    "Light gray",
    "Brown",
    "Rose",
    "Gold",
    "Light yellow",
    "Lime",
    "Light turquoise",
    "Blue-gray",
    "Lavender",
    "Dark gray",
    "Dark brown",
    "Teal",
    "Dark blue",
    "Dark purple",
    "Light orange",
    "Mint",
    "Periwinkle",
];
pub(super) fn tooltip(index: usize, rgb: [u8; 3]) -> String {
    format!(
        "{}\n#{:02X}{:02X}{:02X}",
        NAMES[index], rgb[0], rgb[1], rgb[2]
    )
}
