use std::collections::HashMap;

use codespan_reporting::files::{Files, SimpleFile};

#[derive(Clone, Hash, Debug, PartialEq, Eq, Default, Copy)]
pub struct FileIdent {
    unique_id: usize,
}

impl FileIdent {
    pub const fn new(unique_id: usize) -> Self {
        Self { unique_id }
    }
}

#[derive(Default, Clone)]
pub(crate) struct LoxFiles {
    files: HashMap<FileIdent, SimpleFile<String, String>>,
}

impl<'me> Files<'me> for LoxFiles {
    type FileId = FileIdent;

    type Name = String;

    type Source = &'me str;

    fn name(&'me self, id: Self::FileId) -> Result<Self::Name, codespan_reporting::files::Error> {
        self.files
            .get(&id)
            .ok_or(codespan_reporting::files::Error::FileMissing)
            .map(|f| f.name().clone())
    }

    fn source(
        &'me self,
        id: Self::FileId,
    ) -> Result<Self::Source, codespan_reporting::files::Error> {
        self.files
            .get(&id)
            .ok_or(codespan_reporting::files::Error::FileMissing)
            .map(|f| f.source().as_str())
    }

    fn line_index(
        &'me self,
        id: Self::FileId,
        byte_index: usize,
    ) -> Result<usize, codespan_reporting::files::Error> {
        let f = self.files
            .get(&id)
            .ok_or(codespan_reporting::files::Error::FileMissing)?;
        f.line_index((), byte_index)
    }

    fn line_range(
        &'me self,
        id: Self::FileId,
        line_index: usize,
    ) -> Result<core::ops::Range<usize>, codespan_reporting::files::Error> {
        let f = self.files
            .get(&id)
            .ok_or(codespan_reporting::files::Error::FileMissing)?;
        f.line_range((), line_index)
    }
}

impl LoxFiles {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }

    pub(crate) fn add(&mut self, name: String, content: String) -> FileIdent {
        let unique_id = self.files.len();
        let ident = FileIdent { unique_id };
        self.files
            .insert(ident, SimpleFile::new(name, content));
        ident
    }
}
