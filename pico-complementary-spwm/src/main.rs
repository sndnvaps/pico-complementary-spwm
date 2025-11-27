#![no_std]
#![no_main]

use rp_pico::entry;
//use cortex_m_rt::entry;
use embedded_hal::pwm::SetDutyCycle;
use panic_halt as _;
use rp2040_hal::{
    clocks::{init_clocks_and_plls, Clock},
    pac,
    pwm::Slices,
    sio::Sio,
    watchdog::Watchdog,
};

use rp_pico::XOSC_CRYSTAL_FREQ;

//use lazy_static::lazy_static;

const SAMPLE_POINTS: usize = 100; // 正弦波采样点数
const SINE_FREQ: u32 = 50; // 正弦波频率(Hz)
                           //const PWM_FREQ: u32 = 10000; // PWM载波频率(Hz)
                           //const DEAD_TIME: u16 = 100; // 死区时间计数值

// 预计算互补正弦波数据表
//计划将其变成实际的数据表，即把结果计算出来，方便下面的程序直接取用
static COMPLEMENTARY_SINE_TABLE: [(u16, u16); SAMPLE_POINTS] = {
    let comple_sine_table: [(u16, u16); 100] = [
        (32667, 32668),
        (34724, 30611),
        (36774, 28561),
        (38807, 26528),
        (40816, 24519),
        (42793, 22542),
        (44730, 20605),
        (46619, 18716),
        (48453, 16882),
        (50225, 15110),
        (51927, 13408),
        (53554, 11781),
        (55098, 10237),
        (56553, 8782),
        (57915, 7420),
        (59176, 6159),
        (60334, 5001),
        (61381, 3954),
        (62316, 3019),
        (63133, 2202),
        (63831, 1504),
        (64405, 930),
        (64854, 481),
        (65176, 159),
        (65370, 0),
        (65435, 0),
        (65370, 0),
        (65176, 159),
        (64854, 481),
        (64405, 930),
        (63831, 1504),
        (63133, 2202),
        (62316, 3019),
        (61381, 3954),
        (60334, 5001),
        (59176, 6159),
        (57915, 7420),
        (56553, 8782),
        (55098, 10237),
        (53554, 11781),
        (51927, 13408),
        (50225, 15110),
        (48453, 16882),
        (46619, 18716),
        (44730, 20605),
        (42793, 22542),
        (40816, 24519),
        (38807, 26528),
        (36774, 28561),
        (34724, 30611),
        (32667, 32668),
        (30610, 34725),
        (28560, 36775),
        (26527, 38808),
        (24518, 40817),
        (22541, 42794),
        (20604, 44731),
        (18715, 46620),
        (16881, 48454),
        (15109, 50226),
        (13407, 51928),
        (11780, 53555),
        (10236, 55099),
        (8781, 56554),
        (7419, 57916),
        (6158, 59177),
        (5000, 60335),
        (3953, 61382),
        (3018, 62317),
        (2201, 63134),
        (1503, 63832),
        (929, 64406),
        (480, 64855),
        (158, 65177),
        (0, 65371),
        (0, 65435),
        (0, 65371),
        (158, 65177),
        (480, 64855),
        (929, 64406),
        (1503, 63832),
        (2201, 63134),
        (3018, 62317),
        (3953, 61382),
        (5000, 60335),
        (6158, 59177),
        (7419, 57916),
        (8781, 56554),
        (10236, 55099),
        (11780, 53555),
        (13407, 51928),
        (15109, 50226),
        (16881, 48454),
        (18715, 46620),
        (20604, 44731),
        (22541, 42794),
        (24518, 40817),
        (26527, 38808),
        (28560, 36775),
        (30610, 34725),
    ];
    comple_sine_table
};


#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // 初始化时钟
    let clocks = init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    // 配置GPIO引脚
    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // 配置PWM切片0（主通道和互补通道）
    let pwm_slices_0 = Slices::new(pac.PWM, &mut pac.RESETS);
    // Configure PWM0
    let mut pwm = pwm_slices_0.pwm7;

    // 配置PWM参数
    pwm.set_div_int(1);
    pwm.set_div_frac(0);
    pwm.set_top(65535);

    // 启用双通道输出
    pwm.set_ph_correct();
    pwm.enable();

    // GPIO14作为主PWM输出
    let mut channel_a = pwm.channel_a;
    let _pwm_main_pin = channel_a.output_to(pins.gpio14);

    // GPIO15作为互补PWM输出
    let mut channel_b = pwm.channel_b;
    let _pwm_comp_pin = channel_b.output_to(pins.gpio15);

    // 主循环：生成互补SPWM信号
    loop {
        for &(main_duty, comp_duty) in COMPLEMENTARY_SINE_TABLE.iter() {
            // 设置主通道占空比
            let _ = channel_a.set_duty_cycle(main_duty.into());
            // 设置互补通道占空比
            let _ = channel_b.set_duty_cycle(comp_duty.into());

            // 计算每个采样点的延迟时间
            let delay_us = 1_000_000 / (SINE_FREQ * SAMPLE_POINTS as u32);
            delay.delay_us(delay_us);
        }
    }
}
