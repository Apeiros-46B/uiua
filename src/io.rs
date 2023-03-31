use std::{
    collections::HashMap,
    env, fs,
    io::{stdin, stdout, BufRead, Cursor, Write},
};

use image::{DynamicImage, ImageOutputFormat};
use rand::prelude::*;

use crate::{
    array::Array,
    compile::Assembly,
    grid_fmt::GridFmt,
    value::Value,
    vm::{CallEnv, Env},
    RuntimeError, RuntimeResult,
};

macro_rules! io_op {
    ($((
        $args:literal$(($outputs:expr))?,
        $variant:ident, $name:literal
    )),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum IoOp {
            $($variant),*
        }

        impl IoOp {
            pub fn from_name(s: &str) -> Option<Self> {
                match s {
                    $($name => Some(Self::$variant)),*,
                    _ => None
                }
            }
            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$variant => $name),*
                }
            }
            pub fn args(&self) -> u8 {
                match self {
                    $(IoOp::$variant => $args,)*
                }
            }
            pub fn outputs(&self) -> Option<u8> {
                match self {
                    $($(IoOp::$variant => $outputs.into(),)?)*
                    _ => Some(1)
                }
            }
        }
    };
}

io_op! {
    (1(0), Show, "show"),
    (1(0), Print, "print"),
    (1(0), Println, "println"),
    (0, ScanLn, "scanln"),
    (0, Args, "args"),
    (1, Var, "var"),
    (0, Rand, "rand"),
    (1, FReadStr, "freadstr"),
    (1, FWriteStr, "fwritestr"),
    (1, FReadBytes, "freadbytes"),
    (1, FWriteBytes, "fwritebytes"),
    (1, FLines, "flines"),
    (1, FExists, "fexists"),
    (1, FListDir, "flistdir"),
    (1, FIsFile, "fisfile"),
    (1, Import, "import"),
    (0, Now, "now"),
    (1, ImRead, "imread"),
    (1, ImWrite, "imwrite"),
    (1(0), ImShow, "imshow"),
}

#[allow(unused_variables)]
pub trait IoBackend {
    fn print_str(&mut self, s: &str);
    fn rand(&mut self) -> f64;
    fn show_image(&mut self, image: DynamicImage, env: &Env) -> RuntimeResult {
        Err(env.error("Showing images not supported in this environment"))
    }
    fn scan_line(&mut self) -> String {
        String::new()
    }
    fn import(&mut self, name: &str, env: &Env) -> RuntimeResult<Vec<Value>> {
        Err(env.error("Import not supported in this environment"))
    }
    fn var(&mut self, name: &str) -> Option<String> {
        None
    }
    fn args(&mut self) -> Vec<String> {
        Vec::new()
    }
    fn file_exists(&self, path: &str) -> bool {
        false
    }
    fn list_dir(&self, path: &str, env: &Env) -> RuntimeResult<Vec<String>> {
        Err(env.error("File IO not supported in this environment"))
    }
    fn is_file(&self, path: &str, env: &Env) -> RuntimeResult<bool> {
        Err(env.error("File IO not supported in this environment"))
    }
    fn read_file(&mut self, path: &str, env: &Env) -> RuntimeResult<Vec<u8>> {
        Err(env.error("File IO not supported in this environment"))
    }
    fn write_file(&mut self, path: &str, contents: Vec<u8>, env: &Env) -> RuntimeResult {
        Err(env.error("File IO not supported in this environment"))
    }
}

impl<'a, T> IoBackend for &'a mut T
where
    T: IoBackend,
{
    fn print_str(&mut self, s: &str) {
        (**self).print_str(s)
    }
    fn rand(&mut self) -> f64 {
        (**self).rand()
    }
    fn show_image(&mut self, image: DynamicImage, env: &Env) -> RuntimeResult {
        (**self).show_image(image, env)
    }
    fn scan_line(&mut self) -> String {
        (**self).scan_line()
    }
    fn import(&mut self, name: &str, env: &Env) -> RuntimeResult<Vec<Value>> {
        (**self).import(name, env)
    }
    fn var(&mut self, name: &str) -> Option<String> {
        (**self).var(name)
    }
    fn args(&mut self) -> Vec<String> {
        (**self).args()
    }
    fn file_exists(&self, path: &str) -> bool {
        (**self).file_exists(path)
    }
    fn list_dir(&self, path: &str, env: &Env) -> RuntimeResult<Vec<String>> {
        (**self).list_dir(path, env)
    }
    fn is_file(&self, path: &str, env: &Env) -> RuntimeResult<bool> {
        (**self).is_file(path, env)
    }
    fn read_file(&mut self, path: &str, env: &Env) -> RuntimeResult<Vec<u8>> {
        (**self).read_file(path, env)
    }
    fn write_file(&mut self, path: &str, contents: Vec<u8>, env: &Env) -> RuntimeResult {
        (**self).write_file(path, contents, env)
    }
}

pub struct StdIo {
    imports: HashMap<String, Vec<Value>>,
    rng: SmallRng,
}

impl Default for StdIo {
    fn default() -> Self {
        Self {
            imports: HashMap::new(),
            rng: SmallRng::seed_from_u64(instant::now().to_bits()),
        }
    }
}

impl IoBackend for StdIo {
    fn print_str(&mut self, s: &str) {
        print!("{}", s);
        let _ = stdout().lock().flush();
    }
    fn rand(&mut self) -> f64 {
        self.rng.gen()
    }
    fn scan_line(&mut self) -> String {
        stdin()
            .lock()
            .lines()
            .next()
            .and_then(Result::ok)
            .unwrap_or_default()
    }
    fn import(&mut self, path: &str, _env: &Env) -> RuntimeResult<Vec<Value>> {
        if !self.imports.contains_key(path) {
            let (stack, _) = Assembly::load_file(path)
                .map_err(RuntimeError::Import)?
                .run_with_backend(&mut *self)
                .map_err(RuntimeError::Import)?;
            self.imports.insert(path.into(), stack);
        }
        Ok(self.imports[path].clone())
    }
    fn var(&mut self, name: &str) -> Option<String> {
        env::var(name).ok()
    }
    fn args(&mut self) -> Vec<String> {
        env::args().collect()
    }
    fn file_exists(&self, path: &str) -> bool {
        fs::metadata(path).is_ok()
    }
    fn is_file(&self, path: &str, env: &Env) -> RuntimeResult<bool> {
        fs::metadata(path)
            .map(|m| m.is_file())
            .map_err(|e| env.error(e.to_string()))
    }
    fn list_dir(&self, path: &str, env: &Env) -> RuntimeResult<Vec<String>> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(path).map_err(|e| env.error(e.to_string()))? {
            let entry = entry.map_err(|e| env.error(e.to_string()))?;
            paths.push(entry.path().to_string_lossy().into());
        }
        Ok(paths)
    }
    fn read_file(&mut self, path: &str, env: &Env) -> RuntimeResult<Vec<u8>> {
        fs::read(path).map_err(|e| env.error(e.to_string()))
    }
    fn write_file(&mut self, path: &str, contents: Vec<u8>, env: &Env) -> RuntimeResult {
        fs::write(path, contents).map_err(|e| env.error(e.to_string()))
    }
}

impl IoOp {
    pub(crate) fn run<B: IoBackend>(&self, env: &mut CallEnv<B>) -> RuntimeResult {
        match self {
            IoOp::Show => {
                let s = env.pop(1)?.grid_string();
                env.vm.io.print_str(&s);
                env.vm.io.print_str("\n");
            }
            IoOp::Print => {
                let val = env.pop(1)?;
                env.vm.io.print_str(&val.to_string());
            }
            IoOp::Println => {
                let val = env.pop(1)?;
                env.vm.io.print_str(&val.to_string());
                env.vm.io.print_str("\n");
            }
            IoOp::ScanLn => {
                let line = env.vm.io.scan_line();
                env.push(line);
            }
            IoOp::Args => {
                let args = env.vm.io.args();
                env.push(Array::from_iter(
                    args.into_iter().map(Array::from).map(Value::from),
                ))
            }
            IoOp::Var => {
                let name = env.pop(1)?;
                if !name.is_array() || !name.array().is_chars() {
                    return Err(env.error("Argument to var must be a string"));
                }
                let key: String = name.array().chars().iter().collect();
                let var = env.vm.io.var(&key).unwrap_or_default();
                env.push(var);
            }
            IoOp::Rand => {
                let num = env.vm.io.rand();
                env.push(num);
            }
            IoOp::FReadStr => {
                let path = env.pop(1)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                let contents = String::from_utf8(env.vm.io.read_file(&path, &env.env())?)
                    .map_err(|e| env.error(&format!("Failed to read file: {}", e)))?;
                env.push(contents);
            }
            IoOp::FWriteStr => {
                let path = env.pop(1)?;
                let contents = env.pop(2)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                if !contents.is_array() || !contents.array().is_chars() {
                    return Err(env.error("Contents must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                env.vm
                    .io
                    .write_file(&path, contents.to_string().into_bytes(), &env.env())?;
            }
            IoOp::FReadBytes => {
                let path = env.pop(1)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                let contents = env.vm.io.read_file(&path, &env.env())?;
                let arr = Array::from(contents);
                env.push(arr);
            }
            IoOp::FWriteBytes => {
                let path = env.pop(1)?;
                let contents = env.pop(2)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                if !contents.is_array() {
                    return Err(env.error("Contents must be an array"));
                }
                let path: String = path.array().chars().iter().collect();
                let contents = contents.into_array();
                let contents = if contents.is_numbers() {
                    contents
                        .into_numbers()
                        .into_iter()
                        .map(|n| n as u8)
                        .collect()
                } else if contents.is_bytes() {
                    contents.into_bytes()
                } else {
                    return Err(env.error("Contents must be a numeric array"));
                };
                env.vm.io.write_file(&path, contents, &env.env())?;
            }
            IoOp::FLines => {
                let path = env.pop(1)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                let contents = String::from_utf8(env.vm.io.read_file(&path, &env.env())?)
                    .map_err(|e| env.error(&format!("Failed to read file: {}", e)))?;
                let lines_array =
                    Array::from_iter(contents.lines().map(Array::from).map(Value::from));
                env.push(lines_array);
            }
            IoOp::FExists => {
                let path = env.pop(1)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                let exists = env.vm.io.file_exists(&path);
                env.push(exists);
            }
            IoOp::FListDir => {
                let path = env.pop(1)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                let paths = env.vm.io.list_dir(&path, &env.env())?;
                let paths_array =
                    Array::from_iter(paths.into_iter().map(Array::from).map(Value::from));
                env.push(paths_array);
            }
            IoOp::FIsFile => {
                let path = env.pop(1)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                let is_file = env.vm.io.is_file(&path, &env.env())?;
                env.push(is_file);
            }
            IoOp::Import => {
                let name = env.pop(1)?;
                if !name.is_array() || !name.array().is_chars() {
                    return Err(env.error("Path to import must be a string"));
                }
                let name: String = name.array().chars().iter().collect();
                for value in env.vm.io.import(&name, &env.env())? {
                    env.push(value);
                }
            }
            IoOp::Now => env.push(instant::now()),
            IoOp::ImRead => {
                let path = env.pop(1)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                let bytes = env.vm.io.read_file(&path, &env.env())?;
                let image = image::load_from_memory(&bytes)
                    .map_err(|e| env.error(&format!("Failed to read image: {}", e)))?
                    .into_rgba8();
                let shape = vec![image.height() as usize, image.width() as usize, 4];
                let array = Array::from((shape, image.into_raw()));
                env.push(array);
            }
            IoOp::ImWrite => {
                let path = env.pop(1)?;
                let value = env.pop(2)?;
                if !path.is_array() || !path.array().is_chars() {
                    return Err(env.error("Path must be a string"));
                }
                let path: String = path.array().chars().iter().collect();
                let ext = path.split('.').last().unwrap_or("");
                let output_format = match ext {
                    "jpg" | "jpeg" => ImageOutputFormat::Jpeg(100),
                    "png" => ImageOutputFormat::Png,
                    "bmp" => ImageOutputFormat::Bmp,
                    "gif" => ImageOutputFormat::Gif,
                    "ico" => ImageOutputFormat::Ico,
                    "tga" => ImageOutputFormat::Tga,
                    "tiff" => ImageOutputFormat::Tiff,
                    _ => ImageOutputFormat::Png,
                };
                let bytes = value_to_image_bytes(&value, output_format, &env.env())?;
                env.vm.io.write_file(&path, bytes, &env.env())?;
            }
            IoOp::ImShow => {
                let value = env.pop(1)?;
                let image = value_to_image(&value, &env.env())?;
                env.vm.io.show_image(image, &env.env())?;
            }
        }
        Ok(())
    }
}

pub fn value_to_image_bytes(
    value: &Value,
    format: ImageOutputFormat,
    env: &Env,
) -> RuntimeResult<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    value_to_image(value, env)?
        .write_to(&mut bytes, format)
        .map_err(|e| env.error(format!("Failed to write image: {}", e)))?;
    Ok(bytes.into_inner())
}

pub fn value_to_image(value: &Value, env: &Env) -> RuntimeResult<DynamicImage> {
    if !value.is_array() || value.array().rank() != 3 {
        return Err(env.error("Image must be a rank 3 numeric array"));
    }
    let arr = value.array();
    let bytes = if arr.is_numbers() {
        arr.numbers()
            .iter()
            .map(|f| (*f * 255.0).floor() as u8)
            .collect()
    } else if arr.is_bytes() {
        arr.bytes().to_vec()
    } else {
        return Err(env.error("Image must be a rank 3 numeric array"));
    };
    let [width, height, px_size] = match arr.shape() {
        &[a, b, c] => [a, b, c],
        _ => unreachable!("Shape checked above"),
    };
    Ok(match px_size {
        1 => image::GrayImage::from_raw(width as u32, height as u32, bytes)
            .ok_or_else(|| env.error("Failed to create image"))?
            .into(),
        3 => image::RgbImage::from_raw(width as u32, height as u32, bytes)
            .ok_or_else(|| env.error("Failed to create image"))?
            .into(),
        4 => image::RgbaImage::from_raw(width as u32, height as u32, bytes)
            .ok_or_else(|| env.error("Failed to create image"))?
            .into(),
        n => {
            return Err(env.error(format!(
                "The last dimension of an image array must be 1, 3, or 4, but it is {n}"
            )))
        }
    })
}
