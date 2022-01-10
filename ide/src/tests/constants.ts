import path from 'path';

export const MOCK_PROJECT_DIRECTORY = path.resolve('src/tests/mock_project/');
export const MOCK_PROJECT_FILES = {
    'forward_slice': path.resolve(MOCK_PROJECT_DIRECTORY, 'src/forward_slice.rs'),
    'backward_slice': path.resolve(MOCK_PROJECT_DIRECTORY, 'src/backward_slice.rs'),
    'find_mutations': path.resolve(MOCK_PROJECT_DIRECTORY, 'src/find_mutations.rs'),
};
