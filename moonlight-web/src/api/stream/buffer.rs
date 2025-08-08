use std::string::FromUtf8Error;

pub struct ByteBuffer<T> {
    position: usize,
    limit: usize,
    buffer: T,
}

impl<T> ByteBuffer<T>
where
    T: AsRef<[u8]>,
{
    pub fn new(buffer: T) -> Self {
        Self {
            position: 0,
            limit: 0,
            buffer,
        }
    }

    pub fn get_u8_array(&mut self, array: &mut [u8]) {
        array.copy_from_slice(&self.buffer.as_ref()[self.position..(self.position + array.len())]);
        self.position += array.len();
    }
    pub fn get_u8(&mut self) -> u8 {
        let mut buffer = [0u8; 1];
        self.get_u8_array(&mut buffer);
        buffer[0]
    }

    // TODO: better error?
    // TODO: is this correct?
    pub fn get_utf8(&mut self, characters: usize) -> Result<&str, ()> {
        if characters == 0 {
            return Ok("");
        }

        let Some(chunk) = &self.buffer.as_ref()[self.position..].utf8_chunks().next() else {
            return Err(());
        };
        let Some((end_char_index, end_char)) = chunk.valid().char_indices().nth(characters - 1)
        else {
            return Err(());
        };
        let output = &chunk.valid()[0..end_char_index + (end_char.len_utf8())];

        Ok(output)
    }

    pub fn reset(&mut self) {
        self.position = 0;
        self.limit = 0;
    }
    pub fn flip(&mut self) {
        self.limit = self.position;
        self.position = 0;
    }
}

impl<T> ByteBuffer<T>
where
    T: AsMut<[u8]>,
{
    pub fn put_u8_array(&mut self, array: &[u8]) {
        self.buffer.as_mut()[self.position..].copy_from_slice(array);
    }
    pub fn put_u8(&mut self, data: u8) {
        self.put_u8_array(&[data]);
    }
}
