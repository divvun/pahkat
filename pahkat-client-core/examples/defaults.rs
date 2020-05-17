fn main() {
    println!("Config path: {:?}", pahkat_client::defaults::config_path());
    println!("Tmp path: {:?}", pahkat_client::defaults::tmp_dir());
    println!("Cache path: {:?}", pahkat_client::defaults::cache_dir());

    println!("\n==As root:==");
    println!("Config path: {:?}", pathos::system::app_config_dir("Pahkat"));
    println!("Tmp path: {:?}", pathos::system::app_temporary_dir("Pahkat"));
    println!("Cache path: {:?}", pathos::system::app_cache_dir("Pahkat"));

}
