pub enum Buttons {
    A,
    B,
    Select,
    Start,
    Up,
    Down,
    Left,
    Right,
}

pub struct Controller {
    strobe: bool,
    button_states: [bool; 8],
    read_index: u8,
}

impl Controller {
    pub fn new() -> Controller {
        return Controller{
            strobe: false,
            button_states: [false; 8],
            read_index: 0,
        }
    }

    pub fn set_strobe(&mut self, v: bool) {
        self.strobe = v;
        if v == true {
            self.read_index = 0;
        }
    }

    pub fn read_next_button_state(&mut self) -> u8 {
        if self.strobe == true {
            self.read_index = 0;
            return self.button_states[0] as u8;
        }
        let mut state = 0;
        if self.read_index < 8 {
            state = self.button_states[self.read_index as usize] as u8;
        }
        self.read_index += 1;
        return state;
    }

    pub fn set_button_state(&mut self, button: Buttons, state: bool) {
        match button {
            Buttons::A => {
                self.button_states[0] = state;
            },
            Buttons::B => {
                self.button_states[1] = state;
            },
            Buttons::Select => {
                self.button_states[2] = state;
            },
            Buttons::Start => {
                self.button_states[3] = state;
            },
            Buttons::Up => {
                self.button_states[4] = state;
            },
            Buttons::Down => {
                self.button_states[5] = state;
            },
            Buttons::Left => {
                self.button_states[6] = state;
            },
            Buttons::Right => {
                self.button_states[7] = state;
            },
        };
    }
}
