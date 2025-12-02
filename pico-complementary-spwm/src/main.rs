#![no_std]
#![no_main]

use defmt::info;
use embedded_hal::digital::OutputPin;
use embedded_hal::pwm::SetDutyCycle;
use panic_halt as _;
use rp2040_hal::{
    clocks::{init_clocks_and_plls, Clock},
    gpio::{self, Interrupt},
    pac::{self, interrupt},
    pwm::Slices,
    sio::Sio,
    watchdog::Watchdog,
};
use rp_pico::entry;
// The GPIO interrupt type we're going to generate
use rp2040_hal::gpio::Interrupt::EdgeLow;

use core::cell::RefCell;
use critical_section::Mutex;

use rp_pico::XOSC_CRYSTAL_FREQ;

use core::sync::atomic::{AtomicBool, Ordering};

//初始化pwm_enabled状态为false,开机的时候不输出信号
//需要按下usr_btn后，才输出pwm信号，再次按下usr_btn暂停pwm信号输出
static PWM_ENABLED: AtomicBool = AtomicBool::new(false);

/// This pin will be our interrupt source.
/// It will trigger an interrupt if pulled to ground (via a switch or jumper wire)
/// usr btn defind as gpio24
type ButtonPin = gpio::Pin<gpio::bank0::Gpio24, gpio::FunctionSioInput, gpio::PullUp>;
/// This how we transfer our Led and Button pins into the Interrupt Handler.
/// We'll have the option hold both using the LedAndButton type.
/// This will make it a bit easier to unpack them later.
static GLOBAL_PINS: Mutex<RefCell<Option<ButtonPin>>> = Mutex::new(RefCell::new(None));

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

    // 配置GPIO引脚为button_pin
    let pins = rp2040_hal::gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    //led is in gpio25
    let mut led_pin = pins.gpio25.into_push_pull_output();

    // 配置GPIO按钮并启用中断
    let button_pin = pins.gpio24.reconfigure();
    button_pin.set_interrupt_enabled(Interrupt::EdgeLow, true);

    // Give away our pins by moving them into the `GLOBAL_PINS` variable.
    // We won't need to access them in the main thread again
    critical_section::with(|cs| {
        GLOBAL_PINS.borrow(cs).replace(Some(button_pin));
    });
    unsafe {
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);
    }

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

    //等待中断信号。接收到信号后，再执行主循环
    //开机后，按下usr按键，将启动pwm信号输出
    //启动pwm信号输出后，再按下usr按键，将暂停pwm信号输出
    // 主循环：生成互补SPWM信号
    loop {
        if PWM_ENABLED.load(Ordering::Relaxed) {
            led_pin.set_high().unwrap();
            info!("State changed: Running");
            for &(main_duty, comp_duty) in COMPLEMENTARY_SINE_TABLE.iter() {
                // 设置主通道占空比
                let _ = channel_a.set_duty_cycle(main_duty.into());
                // 设置互补通道占空比
                let _ = channel_b.set_duty_cycle(comp_duty.into());

                // 计算每个采样点的延迟时间
                let delay_us = 1_000_000 / (SINE_FREQ * SAMPLE_POINTS as u32);
                delay.delay_us(delay_us);
            }
        } else {
            led_pin.set_low().unwrap();
            info!("State changed: Paused");
            // 设置主通道占空比为0
            let _ = channel_a.set_duty_cycle(0);
            // 设置互补通道占空比
            let _ = channel_b.set_duty_cycle(0);
        }
        // 短暂延时
        cortex_m::asm::delay(1000);
    }
}

#[allow(static_mut_refs)] // See https://github.com/rust-embedded/cortex-m/pull/561
#[interrupt]
fn IO_IRQ_BANK0() {
    static mut BTN_IRQ: Option<ButtonPin> = None;
    if BTN_IRQ.is_none() {
        critical_section::with(|cs| {
            *BTN_IRQ = GLOBAL_PINS.borrow(cs).take();
        });
    }

    if let Some(gpios) = BTN_IRQ {
        let btn = gpios;
        if btn.interrupt_status(EdgeLow) {
            // 切换PWM使能状态
            let current_state = PWM_ENABLED.load(Ordering::Relaxed);
            PWM_ENABLED.store(!current_state, Ordering::Relaxed);

            // Our interrupt doesn't clear itself.
            // Do that now so we don't immediately jump back to this interrupt handler.
            btn.clear_interrupt(EdgeLow);
            // 简单防抖延迟
            cortex_m::asm::delay(50_000);
        }
    }
}
