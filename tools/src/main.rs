const SAMPLE_POINTS: usize = 100; // 正弦波采样点数
                                  //const SINE_FREQ: u32 = 50; // 正弦波频率(Hz)
                                  //const PWM_FREQ: u32 = 10000; // PWM载波频率(Hz)
const DEAD_TIME: u16 = 100; // 死区时间计数值

fn main() {
    // 预计算互补正弦波数据表
    //计划将其变成实际的数据表，即把结果计算出来，方便下面的程序直接取用

    let mut table = [(0u16, 0u16); SAMPLE_POINTS];
    let mut i = 0;
    while i < SAMPLE_POINTS {
        let angle = 2.0 * 3.1415926535 * i as f32 / SAMPLE_POINTS as f32;
        let sine_value = angle.sin();

        // 主通道占空比（0-65535）
        //32767.5 = 50% * 65535
        let main_duty = ((sine_value + 1.0) * 32767.5) as u16;

        // 互补通道占空比（取反）
        let comp_duty = 65535u16.saturating_sub(main_duty);

        // 应用死区时间
        let main_with_dead = main_duty.saturating_sub(DEAD_TIME);
        let comp_with_dead = comp_duty.saturating_sub(DEAD_TIME);

        table[i] = (main_with_dead, comp_with_dead);
        i += 1;
    }
    //table
    //println!("tables[(u16,u16);sample_points] = {:?}", table);
    let mut i = 0;
    while i < SAMPLE_POINTS {

        println!("{:?},",table[i]);
        i+=1;
    }
}
