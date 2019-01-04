#![warn(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![feature(vec_resize_default)]

// TODO: Remove these later
#![allow(dead_code)]
#![allow(unused_parens)]
#![allow(unused_imports)]

#[cfg(feature="serialize")] #[macro_use] extern crate serde;
#[cfg(feature="serialize")] use serde_cbor::{ser};
#[cfg(feature="serialize")] use libflate::gzip;

#[macro_use] extern crate failure;

// Re-export tiled for use by the game
#[cfg(feature = "tiled_format")]
pub use tiled;

//pub type Vector2<N> = VectorN<N, U2>;
use nalgebra::{
    Vector2,
    Vector3,
    Vector4,
};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Index out of bounds: {} for {}", coordinate, dimensions)]
    OutOfBounds {
        dimensions: Vector2<u32>,
        coordinate: Vector2<u32>,
    },

    #[fail(display = "Internal error in serde_cbor: {:?}", inner)]
    SerdeCBOR {
        inner: serde_cbor::error::Error
    },

    #[fail(display = "Internal IO Error: {:?}", inner)]
    IO {
        inner: std::io::Error,
    },

    #[cfg(feature = "tiled_format")]
    #[cfg_attr(feature="tiled_format", fail(display = "Internal Tiled Error: {:?}", inner))]
    Tiled {
        inner: crate::tiled::TiledError,
    }
}
impl From<serde_cbor::error::Error> for Error {
    fn from(inner: serde_cbor::error::Error) -> Self {
        Error::SerdeCBOR { inner }
    }
}
impl From<std::io::Error> for Error {
    fn from(inner: std::io::Error) -> Self {
        Error::IO { inner }
    }
}
#[cfg(feature = "tiled_format")]
impl From<crate::tiled::TiledError> for Error {
    fn from(inner: crate::tiled::TiledError) -> Self {
        Error::Tiled { inner }
    }
}

type TilesResult<T> = Result<T, Error>;

//#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]

const DEFAULT_SIZE: u32 = 1024;

pub trait Grid2D<T: Send + Sync + Default + Clone> {
    fn dimensions(&self, ) -> Vector2<u32>;

    fn get(&self, coord: Vector2<u32>, ) -> Option<&T>;
    fn get_mut(&mut self, coord: Vector2<u32>, ) -> Option<&mut T>;
    fn set(&mut self, coord: Vector2<u32>, value: T, ) -> TilesResult<()>;

    fn iter_region(&self, region: Vector4<u32>, ) -> GridRegionIter<'_, T>;
    fn iter_region_mut(&mut self, region: Vector4<u32>, ) -> GridRegionIterMut<'_, T>;

    fn iter(&self) -> GridRegionIter<'_, T>;
    fn iter_mut(&mut self) -> GridRegionIterMut<'_, T>;
}

#[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
pub struct GridStorage2D<T: Send + Sync + Default + Clone> {
    storage: Vec<T>,
    dimensions: Vector2<u32>,
}
impl<T: Send + Sync + Default + Clone> Default for GridStorage2D<T> {
    fn default() -> Self {
        Self::new(Vector2::new(DEFAULT_SIZE, DEFAULT_SIZE))
    }
}
impl<T: Send + Sync + Default + Clone> GridStorage2D<T> {
    pub fn new(dimensions: Vector2<u32>) -> Self {
        let capacity = dimensions.x * dimensions.y;
        let mut selfie = Self {
            storage: Vec::with_capacity(capacity as usize),
            dimensions
        };
        selfie.storage.resize(capacity as usize, T::default());
        selfie
    }

    fn flatten(&self, coord: Vector2<u32>) -> usize {
        let i = (coord.x * self.dimensions.x) + coord.y;

        #[cfg(not(feature = "unbounded"))]
            {
                #[allow(clippy::cast_possible_truncation)]
                let capacity = self.storage.capacity() as u32;
                if i > capacity {
                    panic!(Error::OutOfBounds { coordinate: coord, dimensions: self.dimensions})
                }
            }

        i as usize
    }

    pub fn write<W>(&self, writer: &mut W) -> TilesResult<()>
        where W: std::io::Write,
              T: serde::Serialize
    {
        #[allow(clippy::unit_arg)]
        Ok(serde_cbor::to_writer(writer, self)?)
    }

    pub fn write_compressed<W>(&self, writer: W) -> TilesResult<()>
        where W: std::io::Write,
              T: serde::Serialize
    {
        let mut encoder = gzip::Encoder::new(writer)?;
        #[allow(clippy::unit_arg)]
        Ok(serde_cbor::to_writer(&mut encoder, self)?)
    }

}
impl<T: Send + Sync + Default + Clone> Grid2D<T> for GridStorage2D<T> {
    fn dimensions(&self, ) -> Vector2<u32> { self.dimensions }

    fn get(&self, coord: Vector2<u32>, ) -> Option<&T> {
        self.storage.get( self.flatten(coord) )
    }
    fn get_mut(&mut self, coord: Vector2<u32>, ) -> Option<&mut T> {
        let i = self.flatten(coord);
        self.storage.get_mut( i )
    }
    fn set(&mut self, coord: Vector2<u32>, value: T) -> Result<(), Error> {
        let i = self.flatten(coord);

        self.storage[i] = value;

        Ok(())
    }

    fn iter_region(&self, region: Vector4<u32>, ) -> GridRegionIter<'_, T> {
        GridRegionIter::<T>::new(region, self)
    }
    fn iter_region_mut(&mut self, region: Vector4<u32>, ) -> GridRegionIterMut<'_, T> {
        GridRegionIterMut::<T>::new(region, self)
    }

    fn iter(&self) -> GridRegionIter<'_, T> {
        self.iter_region(Vector4::new(0, 0, self.dimensions.x-1, self.dimensions.y-1))
    }
    fn iter_mut(&mut self) -> GridRegionIterMut<'_, T> {
        self.iter_region_mut(Vector4::new(0, 0, self.dimensions.x-1, self.dimensions.y-1))
    }
}

trait IncrementRegionHelper {
    fn increment(region: &Vector4<u32>, current: &mut Vector2<u32>, stride: u32){
        current.x += stride;
        if current.x > region.z {
            current.x = region.x;
            current.y += stride;
        }
    }
}

pub struct GridRegionIter<'a, T: Send + Sync + Clone + Default> {
    region: Vector4<u32>,
    current: Vector2<u32>,
    data: &'a dyn Grid2D<T>,
    stride: u32,
}
impl<'a, T: Send + Sync + Clone + Default> GridRegionIter<'a, T> {
    fn new(region: Vector4 < u32 >, data: &'a dyn Grid2D<T>, ) -> Self {
        Self { region, data, current: Vector2::new(region.x, region.y), stride: 1, }
    }
}
impl<'a, T: Send + Sync + Clone + Default> Iterator for GridRegionIter<'a, T> {
    type Item = (Vector2<u32>, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.data.get(self.current);
        let last = self.current;

        if self.current.y > self.region.w {
            return None;
        }

        Self::increment(&self.region, &mut self.current, self.stride);

        match ret {
            Some(v) => { Some((last, v))},
            None => None,
        }
    }
}
impl<'a, T: Send + Sync + Clone + Default> IncrementRegionHelper for GridRegionIter<'a, T> { }

pub struct GridRegionIterMut<'a, T: Send + Sync> {
    region: Vector4<u32>,
    current: Vector2<u32>,
    data: Option<&'a mut dyn Grid2D<T>>,
    stride: u32,
}
impl<'a, T: Send + Sync + Clone + Default> GridRegionIterMut<'a, T> {
    fn new(region: Vector4 < u32 >, data: &'a mut dyn Grid2D<T>, ) -> Self {
        Self { region, data: Some(data), current: Vector2::new(region.x, region.y), stride: 1, }
    }
}
impl<'a, T: Send + Sync + Clone + Default> Iterator for  GridRegionIterMut<'a, T> {
    type Item = (Vector2<u32>, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.data.take().unwrap().get_mut(self.current);
        let last = self.current;

        if self.current.y > self.region.w {
            return None;
        }

        Self::increment(&self.region, &mut self.current, self.stride);

        match ret {
            Some(v) => { Some((last, v))},
            None => None,
        }
    }
}
impl<'a, T: Send + Sync + Clone + Default> IncrementRegionHelper for GridRegionIterMut<'a, T> { }

/// We require this trait if using the Tiled format, because we fill the data members with this
#[cfg(feature = "tiled_format")]
pub trait Tile {
    fn value(&self) -> u32;
    fn set_value(&mut self, value: u32);
}

#[cfg(feature = "tiled_format")]
pub fn from_tiled<T, R>(reader: R, path: &std::path::Path) -> TilesResult<(crate::tiled::Map, GridStorage2D<T>)>
    where
        R: std::io::Read,
        T:Send + Sync + Clone + Default + Tile
{
    let map = crate::tiled::parse_with_path(reader, path)?;

    let mut grid = GridStorage2D::<T>::new(Vector2::new(map.width, map.height));
    // Read the map, and assign the types

    let mut y = 0;
    let mut x = 0;
    map.layers[0].tiles.iter().for_each(|row| {
        row.iter().for_each(|value| {
            grid.get_mut(Vector2::new(x, y)).unwrap().set_value(*value);
            y += 1;
        });
        x += 1;
        y = 0;
    });

    Ok((map, grid))
}

/// Structure acting as scaffolding for serde when loading a spritesheet file.
/// Positions originate in the top-left corner (bitmap image convention).
#[cfg(feature = "amethyst")]
#[cfg(feature = "serialize")]
pub mod amethyst {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SpritePosition {
        /// Horizontal position of the sprite in the sprite sheet
        pub x: u32,
        /// Vertical position of the sprite in the sprite sheet
        pub y: u32,
        /// Width of the sprite
        pub width: u32,
        /// Height of the sprite
        pub height: u32,
        /// Number of pixels to shift the sprite to the left and down relative to the entity holding it
        pub offsets: Option<[f32; 2]>,
    }

    /// Structure acting as scaffolding for serde when loading a spritesheet file.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct SerializedSpriteSheet {
        /// Width of the sprite sheet
        pub spritesheet_width: u32,
        /// Height of the sprite sheet
        pub spritesheet_height: u32,
        /// Description of the sprites
        pub sprites: Vec<SpritePosition>,
    }

    #[cfg(feature = "tiled_format")]
    pub fn spritesheet_from_tiled<R>(reader: R, path: &std::path::Path) -> TilesResult<Vec<SerializedSpriteSheet>>
        where R: std::io::Read,
    {
        let map = crate::tiled::parse_with_path(reader, path)?;
        let mut sheets = Vec::new();

        map.tilesets.iter().for_each(|sheet| {

            #[allow(clippy::cast_sign_loss)]
            let mut amethyst_sheet = SerializedSpriteSheet {
                spritesheet_width: sheet.images[0].width as u32,
                spritesheet_height: sheet.images[0].height as u32,
                sprites: Vec::new()
            };

            let cols = amethyst_sheet.spritesheet_width / map.tile_width;
            let rows = amethyst_sheet.spritesheet_height / map.tile_height;

            for row in 0..rows {
                for col in 0..cols {
                    amethyst_sheet.sprites.push(SpritePosition {
                        x: col * map.tile_width,
                        y: row * map.tile_height,
                        width: map.tile_width,
                        height: map.tile_height,
                        offsets: None,
                    });
                }
            }

            sheets.push(amethyst_sheet);
        });

        Ok(sheets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "amethyst")]
    #[cfg(feature = "serialize")]
    mod amethyst {

        #[cfg(feature = "tiled_format")]
        #[test]
        fn amethyst_export() {
            let path = std::path::Path::new("/tmp/untitled.tmx");
            let file = std::fs::File::open(&path).unwrap();
            let serialized = crate::amethyst::spritesheet_from_tiled(file, &path).unwrap();

            let pretty = ron::ser::PrettyConfig {
                depth_limit: 99,
                separate_tuple_members: true,
                enumerate_arrays: true,
                ..ron::ser::PrettyConfig::default()
            };
            let s = ron::ser::to_string_pretty(&serialized, pretty).expect("Serialization failed");

            println!("{}", s);
        }
    }




    #[cfg(feature = "tiled_format")]
    #[derive(Clone, Default, Serialize, Deserialize)]
    struct TestTiledData {
        data: u32,
    }
    impl Tile for TestTiledData {
        fn value(&self) -> u32 {
            self.data
        }
        fn set_value(&mut self, value: u32) {
            self.data = value;
        }
    }

    #[cfg(feature = "tiled_format")]
    #[test]
    fn tiled() {
        let path = std::path::Path::new("/tmp/untitled.tmx");
        let file = std::fs::File::open(&path).unwrap();
        let (_tiled_map, grid) = from_tiled::<TestTiledData, std::fs::File>(file, &path).unwrap();

        grid.iter().for_each(|(_, tile)| {
            assert_eq!(tile.value(), 3);
        });
    }

    #[cfg_attr(feature="serialize", derive(Serialize, Deserialize))]
    #[derive(Clone, Default)]
    struct TestData {
        data: u8,
    }

    #[test]
    fn region_iter() {

        let mut grid = GridStorage2D::<TestData>::new(Vector2::new(32, 32));

        // Fill regions with test values
        let mut initial_count = 0;
        for x in 0..6 {
            for y in 0..6 {
                grid.set(Vector2::new(x, y), TestData { data: 1 }).unwrap();
                initial_count += 1;
            }
        }
        let mut count = 0;
        grid.iter_region(Vector4::new(0, 0, 5, 5)).for_each(|(coord, val)| {
            assert!(coord.x <= 5);
            assert!(coord.y <= 5);
            assert_eq!(val.data, 1);
            count += 1;
        });

        assert_eq!(count, initial_count);

    }

    #[test]
    # [cfg(feature = "serialize")]
    fn footprint_random() {
        use rand::prelude::*;
        let mut output_vec = Vec::<u8>::new();

        let mut grid = GridStorage2D::<TestData>::new(Vector2::new(1024, 1024));
        println!("Created buffer of: 1024 * 1024 = {}", 1024 * 1024);

        for x in 0..1023 {
            for y in 0..1023 {
                assert!(grid.set(Vector2::new(x, y), TestData { data: rand::random::<u8>(), } ).is_ok());
            }
        }

        assert!(grid.write_compressed(&mut output_vec).is_ok());
        println!("Wrote size of {}", output_vec.len())
    }

    #[test]
    # [cfg(feature = "serialize")]
    fn footprint_empty() {
        let mut output_vec = Vec::<u8>::new();

        let grid = GridStorage2D::<TestData>::new(Vector2::new(1024, 1024));
        println!("Created buffer of: 1024 * 1024 = {}", 1024 * 1024);

        assert!(grid.write_compressed(&mut output_vec).is_ok());
        println!("Wrote size of {}", output_vec.len())
    }
}