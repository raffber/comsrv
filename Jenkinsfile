pipeline {
    agent {
        dockerfile {
            dir 'jenkins'
        }
    }
    stages {
        stage('test-comsrv') {
            steps {
                sh 'cd comsrv && cargo test'
            }
        }
        stage('build-artifacts') {
            steps {
                sh 'cd comsrv && cargo build --release'
                sh 'cd comsrv && cargo build --target x86_64-pc-windows-gnu --release'
                archiveArtifacts(artifacts: 'comsrv/target/release/comsrv, comsrv/target/x86_64-pc-windows-gnu/release/comsrv.exe')
            }
        }
    }
}
