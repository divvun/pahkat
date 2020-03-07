fn main() {
    println!("Config path: {:?}", pahkat_client::defaults::config_path());
    println!("Tmp path: {:?}", pahkat_client::defaults::tmp_dir());
    println!("Cache path: {:?}", pahkat_client::defaults::cache_dir());
}
