#![no_std]
#![no_main]
use embassy_executor::Spawner;
use embassy_nrf::config::Config;
use {defmt_rtt as _, panic_probe as _};

embassy_nrf::bind_interrupts!(
    struct Irqs {}
);

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    embassy_nrf::reset::release_network_core();
    let config = Config::default();
    let _p = embassy_nrf::init(config);
    defmt::info!("Hello, world!");
}
